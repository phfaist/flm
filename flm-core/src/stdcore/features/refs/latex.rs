//! The LaTeX side of the [`refs`](super) feature: the recomposition rules
//! that write `\ref` and `\hyperref` back as LaTeX for the `latex-recomposed`
//! backend, the helpers with which a construct's own rule writes its labels,
//! and the render options [`RefsRecomposeOptions`] that shape both.
//!
//! The recomposed LaTeX leaves cross-references to LaTeX: nothing is resolved
//! here. A label is written as a LaTeX `\label` around the label's safe label
//! (the key written in place of an FLM label, `ref1`, `ref2`, …; see
//! [`SafeLabels`]), and a `\ref` as the LaTeX hook
//! `\flmRef` around the same safe label, so that LaTeX pairs the two. A
//! `\hyperref` is written as the LaTeX hook `\flmHyperref` around that safe
//! label and the text the author wrote, recomposed as ordinary content. A
//! LaTeX hook is a macro that the recomposed LaTeX writes and the style file
//! defines with a plain default, which a user redefines in the document's
//! preamble to change what the recomposed LaTeX does at that point
//! ([`preamble`]; the rendering
//! guide lists them under [The LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks)).
//!
//! Each recomposition rule is the default for its own element on the
//! `latex-recomposed` backend only; on every other backend, the feature's
//! rules that resolve references ([`REF_RULE`](super::REF_RULE),
//! [`HYPERREF_RULE`](super::HYPERREF_RULE)) stay the default.
//!
//! No rule here writes a label: a `\label` is a field, data that the
//! construct written before it reads, so the construct's own rule writes the
//! labels it read (the heading rule for a heading, the math rule for an
//! equation line) through [`stage_label`] and [`stage_label_group`].
//!
//! # What is written
//!
//! With the default options, `\section{A}\label{sec:a}` and `\ref{sec:a}`
//! become `\flmHeading`, a newline, `\section{A}{\flmLabels{\label{ref1}}}`
//! and `\flmRef{ref1}`.
//! [`REF_COMMAND`](crate::stdcore::backends::latex::preamble::REF_COMMAND)
//! (`\flmRef`) is the *reference hook*: the style file defines it with
//! `\zcref` of the `zref-clever` package, which writes a reference with the
//! name of its target ("Section 2"), as FLM's `\ref` does, while LaTeX's own
//! `\ref` prints the number alone. A user who wants references typeset
//! differently redefines that one macro instead of rendering the document
//! again ([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)).
//! The render option
//! [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! turns the hooks off; the recomposed LaTeX then writes `\zcref[…]{ref1}`
//! itself and requests the package, without translating the options, which
//! the user then writes in the package's own terms.
//!
//! `\hyperref[sec:a]{the \emph{first} part}` becomes
//! `\flmHyperref{ref1}{the \emph{first} part}`: the same safe label, and the
//! author's text lowered through the ordinary rules, so that a user
//! definition, a special character or mathematics inside it is written as it
//! is written anywhere else.
//! [`HYPERREF_COMMAND`](crate::stdcore::backends::latex::preamble::HYPERREF_COMMAND)
//! (`\flmHyperref`) is the hook of such a reference: its default links the
//! text through the `hyperref` package where the document loaded that
//! package, and prints the text alone where it did not, so that whether a
//! recomposed document carries links stays the author's decision
//! ([`hyperref_piece`](crate::stdcore::backends::latex::preamble::hyperref_piece)).
//! With the hooks off, the recomposed LaTeX writes `\hyperref[{ref1}]{…}`
//! itself and requests the package, as it writes `\zcref` for a `\ref`. A
//! `\hyperref` has no options argument, so nothing is translated for it.
//!
//! The rules record their needs as requirements: the definition piece of each
//! hook they write (a unit of LaTeX definitions registered under an
//! identifier `flm.latex.def.<name>`;
//! [`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)),
//! or the LaTeX package they call directly
//! ([`packages`]); the style file
//! written beside the recomposed LaTeX holds what the document required.
//!
//! [`RefsRecomposeOptions`] changes the rest:
//!
//! - The `\ref` options are written back as an optional argument, each entry
//!   `key` or `key={value}`, separated by commas, in the order of the
//!   reference's key-value argument; a `\ref` written without options gets no
//!   optional argument. They are FLM's own options and are written in FLM's
//!   terms, without translation: the default definition of the hook
//!   translates each option that has a counterpart in `zref-clever`
//!   (`noprefix` becomes that package's `noname`, `abbreviated` its
//!   `abbrev=true`) and drops every other one, the reference then printing
//!   its default wording
//!   ([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)
//!   holds the table). A user who redefines `\flmRef` therefore receives the
//!   options as the document wrote them and maps them as their own reference
//!   command needs. The keys and the text of the values are document text,
//!   staged as text nodes so that the emitter escapes them (`x=Fig_1` becomes
//!   `x={Fig\_1}`); a command inside a value is lowered as anywhere else; the
//!   `=`, the braces around a value and the commas are raw nodes
//!   (`flm.pres.latex.raw`, written byte for byte). The emitter writes the
//!   argument plain, `\flmRef[S,x={Figure}]{ref1}`, and as `[{…}]` once a
//!   value holds a command (its rule for an optional argument).
//! - [`protect`](RefsRecomposeOptions::protect) wraps either reference in
//!   `\NoCaseChange{\protect …}`, which keeps it out of a command that changes
//!   the case of its argument (`\MakeUppercase`), and requests the `textcase`
//!   package, which supplies `\NoCaseChange` on every LaTeX kernel. Off by
//!   default. Without it, the whole reference is uppercased:
//!   `\flmRef[noprefix]{ref1}` inside `\MakeUppercase` reaches the hook as
//!   `NOPREFIX`, a key the hook does not translate, so it is dropped and the
//!   name the author left out comes back; and the safe label itself is
//!   uppercased, so LaTeX looks for a label `REF1`, finds none, and prints
//!   `??` with only a warning. Both failures pass without an error.
//! - [`safe_label_types`](RefsRecomposeOptions::safe_label_types) marks a
//!   label type as [`Raw`](SafeLabelMode::Raw): its labels are written as the
//!   document wrote them (`sec:intro`), byte for byte, and no safe label is
//!   allocated for them. Every type not listed is
//!   [`Safe`](SafeLabelMode::Safe). A text with no type (`\label{intro}`, no
//!   colon) belongs to no type, so the option cannot select it: it is
//!   allocated a safe label and written like any other, and the label scan
//!   reports it as not spelled `type:label`.
//!
//! [`label_spelling`] tells the two cases apart (a safe label, or a label
//! written raw), and [`stage_spelling`] stages (builds into the lowered tree;
//! see [`Lowerer`]) each: a safe label as document text, which the emitter
//! escapes, and a raw label as a raw node.
//! [`stage_label`] writes `\label` around a spelling, and
//! [`stage_label_group`] wraps a group of labels in `\flmLabels`. The rules of
//! the headings, math, theorems, lists, defterm, floats and xnotes features
//! use both, so that a label is written the same way wherever it occurs. Both read the
//! safe-labels table, so a rule that calls them declares that it reads
//! [`SAFE_LABELS`].
//!
//! # A target that holds a template placeholder
//!
//! A user definition kept as a LaTeX definition is written once into the
//! style file, its template lowered unchanged, placeholders included. A
//! reference in that template whose target is fixed (`(see \ref{sec:intro})`)
//! is written as any reference is: around the label's safe label or, where
//! the target is a label the document never mentions, around a value name of
//! the template's synthesized value ([`value_name`](super::value_name)). A
//! target that points at nothing is reported as the document's own are, at
//! each use site that stages the definition, the template being the
//! caller's text for the document.
//!
//! A reference whose target holds a placeholder (`(see \ref{#1})`) points at
//! a label that only a call supplies, and a safe label is allocated from a
//! label's characters, which are not known before the call. So the rules
//! read the target as characters at lowering ([`read_target_at_lowering`]),
//! which fails for such a target, and each use site of the definition is then
//! expanded instead of written as a call: its reference is written around the
//! safe label of the label that use site supplies, as anywhere else. This
//! holds for `\ref`, `\hyperref` and the math feature's `\eqref` alike. A
//! target that a use site completes into a label that nothing carries is
//! reported and still written with a key: the [value
//! catch-up](crate::guide::concepts_overview#value-catch-up) of the expanded
//! use site resolves its references against the document's labels, reports
//! them as the document's own references are reported, and gives every
//! target that has no safe label in the document a value name
//! ([`value_name`](super::value_name)). [`target_spelling`] therefore falls
//! back to a target's own characters only for a label of a type written raw.
//!
//! A label in such a template is a different case: it declares a fact about
//! the document, and the definition is refused
//! ([`userdefs::kept`](crate::stdcore::features::userdefs::kept)).
//!
//! # Labels are written in groups, inside `\flmLabels` when the hooks are on
//!
//! `\flmLabels{…}` is the *labels hook*: outside mathematics it runs the
//! `\label` commands as they are written, and on a line of a display-math
//! environment it makes every label after the first
//! resolve to that line's number, which `amsmath` alone does not do
//! ([`labels_piece`](crate::stdcore::backends::latex::preamble::labels_piece)).
//! The hook wraps a *group* of labels, so the rule that owns the group writes
//! it (the heading rule for a heading's label fields, the math rule for an
//! equation line's labels) through [`stage_label_group`], around the bare
//! `\label` commands that [`stage_label`] writes. No rule writes a label in
//! any other way: a `\label` is a field, and a field node that reached a
//! backend on its own would be an error. A feature that declares a labeled
//! construct of its own calls both helpers in its own recomposition rule.
//! With the hooks on, every label of the recomposed LaTeX is then written
//! inside `\flmLabels`; with the render option
//! [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! off, the same groups are written as bare `\label` commands.
//!
//! This also holds in a registry built without this feature. The safe-labels
//! table is then absent, so [`label_spelling`] writes the label as the
//! document wrote it rather than a safe label, but the group and the hook
//! around it are the same: a definition piece of the LaTeX family
//! ([`preamble::LABELS`]) defines `\flmLabels`, not this feature. Such a
//! registry can also ask that an
//! equation line's labels be left out of the output altogether
//! ([`math_passthrough::latex`](crate::math_passthrough::latex)).

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::{BuildId, NodeId, NodeRef, NodeTree};
use techy::extract::parse_keyval;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    chars_at_lowering,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::aspects::Label;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexMacroPayload, LatexRawPayload,
};
use crate::stdcore::backends::latex::{packages, preamble};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::lower_arg_named_or_nothing;
use crate::stdcore::features::numbering::PrefixWords;

use super::elements::{
    ABBREVIATED_VARIANT, CAPITAL_VARIANT, HYPERREF, LOWERCASE_VARIANT, OPTIONS_ARG, REF, TARGET_ARG,
    TEXT_ARG, reference_targets,
};
use super::safe_labels::{
    REF_DOMAIN, SAFE_LABELS, SafeLabels, VALUE_SAFE_LABELS, ValueSafeLabels, label_key,
};

/// The name of the recomposition rule of `\ref`,
/// `flm.ref.lower.latex-recomposed`, whose implementation is
/// [`RefLatexRule`]: the default for `flm.ref` on the `latex-recomposed`
/// backend.
pub const REF_LATEX_RULE: &str = "flm.ref.lower.latex-recomposed";

/// The name of the recomposition rule of `\hyperref`,
/// `flm.hyperref.lower.latex-recomposed`, whose implementation is
/// [`HyperrefLatexRule`]: the default for `flm.hyperref` on the
/// `latex-recomposed` backend.
pub const HYPERREF_LATEX_RULE: &str = "flm.hyperref.lower.latex-recomposed";

/// The LaTeX label command, `label`: the command in which [`stage_label`]
/// writes a label, inside the labels hook `\flmLabels` when the hooks are on
/// (the render option
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)).
pub const LABEL_COMMAND: &str = "label";

/// The reference command of the `zref-clever` package, `zcref`, which the
/// recomposed LaTeX writes in place of the LaTeX hook `\flmRef` when the label
/// hooks are off (the render option
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)).
/// It is the command that the hook's default definition calls.
pub const ZCREF_COMMAND: &str = "zcref";

/// How the recomposed LaTeX writes the labels of one label type: by their
/// safe label, or as the document wrote them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SafeLabelMode {
    /// The safe label allocated for the label (`ref3`). The default.
    #[default]
    Safe,
    /// The label as the document wrote it (`sec:intro`), byte for byte; no
    /// safe label is allocated for it.
    Raw,
}

/// The render options of the `refs` feature's recomposition rules: how the
/// recomposed LaTeX writes labels and references (see the module
/// documentation). An option type ([`crate::model::options`]) declared with
/// [`Bind::Render`](crate::model::options::Bind::Render): its value is set per
/// render request.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::{RefsRecomposeOptions, SafeLabelMode};
///
/// let options = RefsRecomposeOptions::default();
/// assert!(options.safe_label_types.is_empty());
/// assert!(!options.protect);
/// let raw_sections = RefsRecomposeOptions {
///     safe_label_types: [(String::from("sec"), SafeLabelMode::Raw)].into_iter().collect(),
///     ..RefsRecomposeOptions::default()
/// };
/// assert_eq!(raw_sections.safe_label_types["sec"], SafeLabelMode::Raw);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct RefsRecomposeOptions {
    /// Per label type (`sec`, `eq`), how its labels are written; a type not
    /// listed is [`Safe`](SafeLabelMode::Safe). Empty by default. A label
    /// with no type belongs to none of these and is always written by its
    /// safe label.
    ///
    /// **You own this: a type set to [`Raw`](SafeLabelMode::Raw).** Its labels
    /// are written as the document spelled them instead of as a generated
    /// safe label. You must ensure that those labels are spellings LaTeX can
    /// carry through its auxiliary file: a label holding a character LaTeX
    /// treats specially cannot be read back, which the safe labels exist to
    /// avoid.
    pub safe_label_types: BTreeMap<String, SafeLabelMode>,
    /// Whether a reference is wrapped in `\NoCaseChange{\protect …}`, which
    /// keeps it out of a command that changes the case of its argument
    /// (`\MakeUppercase`). Off by default; when on, the style file requests
    /// the `textcase` package, which supplies `\NoCaseChange` on every LaTeX
    /// kernel.
    ///
    /// A reference left unwrapped inside such a command is uppercased whole,
    /// its options and its safe label alike, and it fails in two ways,
    /// neither of which raises an error.
    /// `\MakeUppercase{… \flmRef[noprefix]{ref1} …}` passes the hook
    /// `NOPREFIX`, a key it does not translate, so it is dropped and the name
    /// the author left out comes back. And the safe label itself is
    /// uppercased, so LaTeX looks for a label `REF1`, finds none, and prints
    /// `??` with only a warning. Turn this on for a document whose headings
    /// or captions are uppercased.
    pub protect: bool,
}

/// What is written for one label or reference target, as [`label_spelling`]
/// and [`target_spelling`] return it: a safe label, or the label as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelSpelling {
    /// The safe label allocated for the label (`ref1`), or a value name
    /// (`ref-v1`).
    Safe(String),
    /// The label as written, byte for byte: a label of a type written raw,
    /// or a label in a registry without the safe-labels table.
    Raw(String),
}

impl LabelSpelling {
    /// The text written, in either case.
    pub fn text(&self) -> &str {
        match self {
            LabelSpelling::Safe(text) | LabelSpelling::Raw(text) => text,
        }
    }
}

/// What to write for a label or a reference target, from the safe-labels
/// table `table`: the safe label the table holds for it (the safe-labels
/// stage allocates one for a well-formed label and for a text that is not
/// spelled `type:label` alike), or the label as written when the table holds
/// none: a label of a type written raw, or any label when the table is empty
/// because no safe-labels stage ran (a registry without this feature).
///
/// `text` is the label as the document wrote it and `label` its parsed form,
/// if it is one. A rule that writes a reference target calls
/// [`target_spelling`] instead, which also reads the value names.
pub fn label_spelling(table: &SafeLabels, text: &str, label: Option<&Label>) -> LabelSpelling {
    match table.safe_name(REF_DOMAIN, &label_key(text, label)) {
        Some(name) => LabelSpelling::Safe(String::from(name)),
        None => LabelSpelling::Raw(match label {
            Some(label) => label.to_string(),
            None => String::from(text),
        }),
    }
}

/// What to write for a target of a reference in the node being lowered: the
/// safe label that the document's safe-labels table holds for it
/// ([`label_spelling`]), else the value name that the synthesized value
/// holding the node gives it ([`VALUE_SAFE_LABELS`], written over a value
/// spliced at lowering), else the target as written: a label of a type
/// written raw, or any target when the rule reads the tables optionally and
/// the plan holds neither.
///
/// Every rule that writes the target of a reference site calls it: those of
/// `\ref`, `\hyperref`, the math feature's `\eqref`, the defterm feature's
/// term link and the theorems feature's back-reference. A rule that calls it
/// declares that it reads both tables, the value names optionally where it
/// reads the document's safe labels optionally.
///
/// # Errors
///
/// Returns [`LowerError`] when the calling rule did not declare the reads.
pub fn target_spelling(
    lw: &dyn Lowerer,
    text: &str,
    label: Option<&Label>,
) -> Result<LabelSpelling, LowerError> {
    static EMPTY: SafeLabels =
        SafeLabels { by_label: BTreeMap::new(), by_name: BTreeMap::new(), allocated: 0 };
    let document = lw.get(SAFE_LABELS)?.unwrap_or(&EMPTY);
    let spelling = label_spelling(document, text, label);
    if let LabelSpelling::Raw(_) = spelling {
        let value: Option<&ValueSafeLabels> = lw.get(VALUE_SAFE_LABELS)?;
        let key = label_key(text, label);
        if let Some(name) = value.and_then(|value| value.safe_name(lw.addr(), &key)) {
            return Ok(LabelSpelling::Safe(String::from(name)));
        }
    }
    Ok(spelling)
}

/// Stages a spelling: a safe label becomes a text node, which the emitter
/// escapes like document text; a raw label becomes a raw node
/// (`flm.pres.latex.raw`), written byte for byte. Returns that one node, as
/// the content of a node slot of a LaTeX macro construct (techy's `ParsedSlot`,
/// a part of a node's content outside its arguments; the construct holds the
/// content of each argument in one).
///
/// # Errors
///
/// Returns [`LowerError`] when the node could not be built, or when the
/// calling rule did not declare that it produces `flm.pres.latex.raw`.
pub fn stage_spelling(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    spelling: &LabelSpelling,
) -> Result<Vec<BuildId>, LowerError> {
    let node = match spelling {
        LabelSpelling::Safe(text) => lw.text(cx, text)?,
        LabelSpelling::Raw(text) => lw.pres_with(
            cx,
            &ElementName::from(RAW),
            LatexRawPayload::new(text.as_str()),
            Vec::new(),
        )?,
    };
    Ok(vec![node])
}

/// The safe-labels table, or an empty table where the plan holds none.
fn safe_labels(lw: &dyn Lowerer) -> Result<&SafeLabels, LowerError> {
    static EMPTY: SafeLabels =
        SafeLabels { by_label: BTreeMap::new(), by_name: BTreeMap::new(), allocated: 0 };
    Ok(lw.get(SAFE_LABELS)?.unwrap_or(&EMPTY))
}

/// Whether the labels and references of this render are written through the
/// LaTeX hooks, read from the backend's render options.
fn label_hooks(lw: &dyn Lowerer) -> Result<bool, LowerError> {
    Ok(lw.option::<LatexRecomposedRenderOptions>()?.label_hooks)
}

/// Stages the options of the `\ref` node `node` as the content of an
/// optional argument (see the module documentation): for each key-value
/// entry with a key, the key as a text node, then, when the entry has a
/// value, a raw node `={`, the value's nodes (its text as text nodes, any
/// other node lowered as the node it was read from) and a raw node `}`; the
/// entries separated by raw nodes `,`. Returns `None` when the reference has
/// no options argument or the argument holds no entry. The value of a lone
/// brace group is the group's content, as LaTeX reads a key-value value.
///
/// The calling rule must declare that it produces `flm.pres.latex.raw`.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be built or a value's node
/// could not be lowered.
pub fn stage_ref_options(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
) -> Result<Option<Vec<BuildId>>, LowerError> {
    let Ok(Some(content)) = node.argument_content_nodes_named(OPTIONS_ARG) else {
        return Ok(None);
    };
    // Each output node of the keyval parse is annotated with the node of the
    // document it copies, or nothing for a piece of text cut at a `,` or `=`.
    let keyvals = match parse_keyval(content, |part| {
        (!part.is_partial()).then(|| part.original().map(|original| original.id())).flatten()
    }) {
        Ok(keyvals) => keyvals,
        Err(_) => return Ok(None),
    };
    let tree = node.tree();
    let raw = |cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str| {
        lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
    };
    let mut nodes = Vec::new();
    for entry in keyvals.iter().filter(|entry| !entry.key().is_empty()) {
        if !nodes.is_empty() {
            nodes.push(raw(cx, lw, ",")?);
        }
        nodes.push(lw.text(cx, entry.key())?);
        let Some(value) = entry.value_content() else { continue };
        nodes.push(raw(cx, lw, "={")?);
        for piece in value.iter() {
            if let Some(text) = piece.chars() {
                nodes.push(lw.text(cx, text)?);
                continue;
            }
            let original: Option<NodeId> = *piece.annotation();
            let index = original.map(|id| id.index() as u32);
            let Some(slice) = index.and_then(|index| tree.slice(index..index + 1)) else {
                return Err(LowerError::structure(
                    "a keyval value node that copies no node of the document",
                ));
            };
            nodes.extend(lw.lower_nodes(cx, slice)?);
        }
        nodes.push(raw(cx, lw, "}")?);
    }
    Ok((!nodes.is_empty()).then_some(nodes))
}

/// Stages one `\label` for a label a construct read (a heading's `\label`
/// field, a label of an equation line): `\label` as a LaTeX macro construct
/// whose one mandatory argument holds the label's spelling
/// ([`label_spelling`] over the safe-labels table, staged by
/// [`stage_spelling`]). Returns the staged node.
///
/// `text` is the label as the document wrote it and `label` its parsed form,
/// as [`label_uses`](super::label_uses) returns them for a field.
///
/// The label is written bare; pass the labels of one group to
/// [`stage_label_group`], which wraps them in the labels hook.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be built, when the calling
/// rule did not declare that it reads `flm.labels.safe`, or when it did not
/// declare that it produces `flm.pres.latex.macro` and
/// `flm.pres.latex.raw`.
pub fn stage_label(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    text: &str,
    label: Option<&Label>,
) -> Result<BuildId, LowerError> {
    let spelling = label_spelling(safe_labels(lw)?, text, label);
    let target = stage_spelling(cx, lw, &spelling)?;
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(LABEL_COMMAND, [LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), target)],
    )
}

/// Wraps a group of `\label` nodes, already staged by [`stage_label`], in the
/// labels hook `\flmLabels{…}`, and requires the definition piece that
/// defines the hook ([`preamble::LABELS`]).
///
/// Every group of labels in a recomposed LaTeX document goes through this
/// function (a heading's label fields, the labels of one line of a
/// display-math environment), so that a user changes what a group of labels
/// does by redefining `\flmLabels`, and so that every label after the first
/// on a line of mathematics resolves. A caller may have no labels to write at
/// all: the math rule leaves an equation line's labels out entirely where the
/// render options ask it to in a registry without this feature
/// ([`math_passthrough::latex`](crate::math_passthrough::latex) says when).
///
/// `labels` is the group's staged nodes, which the hook holds between its
/// braces; the result is the node to stage in their place. With
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
/// off, `labels` is returned unchanged and nothing is required: the
/// recomposed LaTeX then holds bare `\label` commands.
///
/// An empty group is wrapped too when `empty_group` is true — the heading
/// rule writes `{\flmLabels{}}` for a heading with no label, one shape for
/// every heading — and is given back empty when it is false, so that a line
/// of mathematics with no label carries nothing.
///
/// # Errors
///
/// Returns [`LowerError`] when the node could not be built, when the render
/// options could not be read, or when the calling rule did not declare that
/// it produces `flm.pres.latex.macro`.
pub fn stage_label_group(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    labels: Vec<BuildId>,
    empty_group: bool,
) -> Result<Vec<BuildId>, LowerError> {
    if !label_hooks(lw)? || (labels.is_empty() && !empty_group) {
        return Ok(labels);
    }
    lw.require(&Name::from(preamble::LABELS));
    let group = lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(preamble::LABELS_COMMAND, [LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), labels)],
    )?;
    Ok(vec![group])
}

/// Stages `\flmLabelsWithDisplayText{<display text>}{<labels>}`, the LaTeX
/// hook that makes a group of labels refer to a place LaTeX does not number,
/// with the display text as what a reference to them prints
/// ([`label_display_text_piece`](preamble::label_display_text_piece)), and
/// requires the definition pieces of the hook and of the reference type that
/// prints the text alone. Returns the staged node.
///
/// `text` is the display text, already lowered by the caller as ordinary
/// content: the place's own words as the author wrote them (a defined term,
/// an unnumbered float's caption, or the alternative text written in the
/// caption's brackets), lowered exactly as where they are written, an xnote
/// in them included, in the same way as the default heading hook reuses a
/// heading's title. LaTeX typesets the display text at every reference, so a
/// footnote in it is typeset again there; for a float, the author avoids this
/// by writing the caption's alternative text without the footnote.
/// `group` is the labels, already wrapped by [`stage_label_group`].
///
/// The definition rule of the `defterm` feature and the float rule of the
/// `floats` feature call it; no other rule writes this hook.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be built, or when the calling
/// rule did not declare that it produces `flm.pres.latex.macro`.
pub fn stage_labels_with_display_text(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    text: Vec<BuildId>,
    group: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    lw.require(&Name::from(preamble::LABEL_DISPLAY_TEXT));
    lw.require(&Name::from(preamble::ZREF_CLEVER_TYPE));
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(
            preamble::LABELS_WITH_DISPLAY_TEXT_COMMAND,
            [LatexArgShape::Mandatory, LatexArgShape::Mandatory],
        ),
        vec![(String::from(ARG_SLOT), text), (String::from(ARG_SLOT), group)],
    )
}

/// Returns the keys and words of a `zref-clever` reference type, which
/// [`stage_reference_type_setup`] writes as
/// `\zcRefTypeSetup{<type>}{<key>={<word>},…}`. `words` holds the prefix
/// words (such as `Theorem`) that FLM's other outputs write before the number
/// of a reference to the type's targets. A feature that declares a
/// `zref-clever` reference type for its numbered constructs calls it; the
/// theorems and floats features do.
///
/// The keys are the package's, each mapped to the variant of `words` that
/// FLM itself writes where the recomposed LaTeX's reference hook asks the
/// package for that key
/// ([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)):
///
/// | key | words |
/// |---|---|
/// | `Name-sg`, `Name-pl` — printed for `S` and `capital` | the [`CAPITAL_VARIANT`], else the base words |
/// | `name-sg`, `name-pl` — printed by default and for `lowercase` | the [`LOWERCASE_VARIANT`], else the base words |
/// | `Name-sg-ab`, `name-sg-ab`, `Name-pl-ab`, `name-pl-ab` — printed for `abbreviated` | the [`ABBREVIATED_VARIANT`], where `words` declares it; the keys are left out otherwise |
///
/// The abbreviated pair of words serves the capitalized and the lowercase
/// abbreviated keys alike. Each word is given without the tie that joins it
/// to a number on FLM's outputs (a trailing `~`) and without the whitespace
/// around it (`Fig.~` gives `Fig.`), since `zref-clever` places its own space
/// between the name and the number. The words are FLM text, which the caller
/// compiles.
///
/// The keys come in the order of the table: `Name-sg`, `name-sg`, `Name-pl`,
/// `name-pl`, then the four abbreviated keys.
pub fn reference_type_words(words: &PrefixWords) -> Vec<(&'static str, &str)> {
    let base = &words.base;
    let capital = words.variants.get(CAPITAL_VARIANT).unwrap_or(base);
    let lowercase = words.variants.get(LOWERCASE_VARIANT).unwrap_or(base);
    let mut keys = vec![
        ("Name-sg", untied(&capital.one)),
        ("name-sg", untied(&lowercase.one)),
        ("Name-pl", untied(&capital.other)),
        ("name-pl", untied(&lowercase.other)),
    ];
    if let Some(abbreviated) = words.variants.get(ABBREVIATED_VARIANT) {
        let (one, other) = (untied(&abbreviated.one), untied(&abbreviated.other));
        keys.extend([("Name-sg-ab", one), ("name-sg-ab", one), ("Name-pl-ab", other), ("name-pl-ab", other)]);
    }
    keys
}

/// `word` without the tie that joins it to a number on FLM's own outputs (a
/// trailing `~`) and without the whitespace around it: `Fig.~` gives `Fig.`.
fn untied(word: &str) -> &str {
    let word = word.trim();
    word.strip_suffix('~').unwrap_or(word).trim_end()
}

/// Stages `\zcRefTypeSetup{<type>}{<key>={<word>},…}`, the declaration of
/// the `zref-clever` reference type `type_name`, with its keys and words in
/// the order of `keys` (the keys of [`reference_type_words`], each word
/// compiled as FLM content, which is lowered here), and requires the package.
/// Returns the staged nodes. The caller places the declaration where it
/// belongs in the style file, with any condition it is guarded by or any
/// text that follows it.
///
/// # Errors
///
/// Returns [`LowerError`] when a word could not be lowered or a node could
/// not be built, or when the calling rule did not declare that it produces
/// `flm.pres.latex.raw`.
pub fn stage_reference_type_setup(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    type_name: &str,
    keys: &[(&'static str, Arc<NodeTree<Flm>>)],
) -> Result<Vec<BuildId>, LowerError> {
    lw.require(&Name::from(packages::ZREF_CLEVER));
    let raw = |cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str| {
        lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
    };
    let mut body = vec![raw(cx, lw, &format!("\\zcRefTypeSetup{{{type_name}}}{{"))?];
    for (at, (key, word)) in keys.iter().enumerate() {
        let separator = if at == 0 { "" } else { "," };
        body.push(raw(cx, lw, &format!("{separator}{key}={{"))?);
        body.extend(lw.splice(cx, word)?);
        body.push(raw(cx, lw, "}")?);
    }
    body.push(raw(cx, lw, "}")?);
    Ok(body)
}

/// The implementation of [`REF_LATEX_RULE`]: it writes the reference hook
/// `\flmRef` (or `\zcref` with the hooks off) around the spelling of each
/// target, separated by commas, with the options as an optional argument,
/// wrapped in `\NoCaseChange` when [`protect`](RefsRecomposeOptions::protect)
/// is on. See the module documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct RefLatexRule;

impl Lower for RefLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let options = lw.option::<RefsRecomposeOptions>()?.clone();
        let hooks = label_hooks(lw)?;
        let target = staged_target(cx, lw, &node)?;
        // With the hook, the recomposed LaTeX names `\flmRef` and the style file
        // says what it does; without it, the recomposed LaTeX writes the command
        // the hook's default would have run and requests its package itself.
        let command = if hooks {
            lw.require(&Name::from(preamble::REFS));
            preamble::REF_COMMAND
        } else {
            lw.require(&Name::from(packages::ZREF_CLEVER));
            ZCREF_COMMAND
        };
        let mut shapes = Vec::new();
        let mut slots = Vec::new();
        if let Some(keyvals) = stage_ref_options(cx, lw, &node)? {
            shapes.push(LatexArgShape::Optional);
            slots.push((String::from(ARG_SLOT), keyvals));
        }
        shapes.push(LatexArgShape::Mandatory);
        slots.push((String::from(ARG_SLOT), target));
        let mut reference = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(command, shapes),
            slots,
        )?;
        if options.protect {
            reference = protected(cx, lw, reference)?;
        }
        Ok(Lowering::Emit(vec![reference]))
    }
}

/// Stages the targets of a reference node (a `\ref` or a `\hyperref`) as the
/// content of its target argument: for each target, the safe label that the
/// safe-labels stage allocated, or the value name a synthesized value spliced
/// at lowering gives it, or the label as the document wrote it where its type
/// is written raw ([`target_spelling`], [`stage_spelling`]); several targets
/// are separated by commas. A node whose target could not be read stages
/// nothing; a target holding a template's placeholder cannot be written
/// ([`read_target_at_lowering`]).
///
/// # Errors
///
/// Returns [`LowerError`] when the target holds a template's placeholder
/// ([`LowerError::PlaceholderCharacters`]), when the safe-labels table cannot
/// be read, when a node could not be built, or when the calling rule did not
/// declare that it produces `flm.pres.latex.raw`.
fn staged_target(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
) -> Result<Vec<BuildId>, LowerError> {
    read_target_at_lowering(node)?;
    let targets = reference_targets(node).unwrap_or_default();
    let mut out = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        if index > 0 {
            out.push(lw.text(cx, ",")?);
        }
        let spelling = target_spelling(lw, &target.text, target.label.as_ref())?;
        out.extend(stage_spelling(cx, lw, &spelling)?);
    }
    Ok(out)
}

/// Checks that the target of the reference node `node` (its argument named
/// [`TARGET_ARG`]) can be written; it returns nothing of the target. It reads
/// the target as characters at lowering ([`chars_at_lowering`]), which writing
/// it requires, since a label's safe label is looked up by its characters,
/// and discards them; the rule then reads the target through the readers the
/// label scan ([`SCAN`](super::SCAN)) uses. Nothing is read where the node has
/// no such argument, and
/// content that is not characters passes, the target then pointing at
/// nothing, as the label scan reads it.
///
/// Every recomposition rule of a reference command calls it before it writes
/// its target: those of `\ref`, `\hyperref` and the math feature's
/// `\eqref`.
///
/// # Errors
///
/// Returns [`LowerError::PlaceholderCharacters`] for a target holding a
/// placeholder of a template lowered unchanged, whose characters only a call
/// supplies: the user definition's use sites are then expanded (see the
/// [module documentation](self#a-target-that-holds-a-template-placeholder)).
pub fn read_target_at_lowering(node: &NodeRef<'_, Flm>) -> Result<(), LowerError> {
    if let Ok(Some(content)) = node.argument_content_nodes_named(TARGET_ARG) {
        // Characters or not, the target is then read by the label scan's
        // own readers; only the outcome for a placeholder acts here.
        let _characters = chars_at_lowering(content.iter())?;
    }
    Ok(())
}

/// Wraps a staged reference in `\NoCaseChange{\protect …}`, which keeps it
/// out of a command that changes the case of its argument, and requests the
/// package that supplies `\NoCaseChange`: what
/// [`protect`](RefsRecomposeOptions::protect) asks for, written the same way
/// for either reference command.
///
/// The LaTeX kernel defines `\NoCaseChange` from its 2022 release on, and the
/// `textcase` package supplies it on every kernel, so the rule requests that
/// package.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be built, or when the calling
/// rule did not declare that it produces `flm.pres.latex.macro` and
/// `flm.pres.latex.raw`.
fn protected(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    reference: BuildId,
) -> Result<BuildId, LowerError> {
    lw.require(&Name::from(packages::TEXTCASE));
    let protect =
        lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new("\\protect"), Vec::new())?;
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(preamble::NO_CASE_CHANGE, [LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), vec![protect, reference])],
    )
}

/// The implementation of [`HYPERREF_LATEX_RULE`]: it writes the hook
/// `\flmHyperref` (or the `hyperref` package's `\hyperref` with the hooks
/// off) around the target's spelling and the author's text, wrapped in
/// `\NoCaseChange` when [`protect`](RefsRecomposeOptions::protect) is on. See
/// the module documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct HyperrefLatexRule;

impl Lower for HyperrefLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let options = lw.option::<RefsRecomposeOptions>()?.clone();
        let hooks = label_hooks(lw)?;
        let target = staged_target(cx, lw, &node)?;
        // The author's text is content of the document and is recomposed as
        // content: a text a parse recovery left absent writes an empty group
        // and is not reported a second time.
        let shown = lower_arg_named_or_nothing(cx, lw, &node, TEXT_ARG)?;
        // With the hook, the recomposed LaTeX names `\flmHyperref` and the
        // style file says what it does — link the text through `hyperref`
        // where the document loaded that package, print it alone where it did
        // not. Without the hook, the recomposed LaTeX writes the package's own
        // command and requests the package itself, so the choice the hook
        // leaves to the author is made here.
        // The two identifiers are spelled alike and are different things:
        // `preamble::HYPERREF` is the definition piece that defines the hook
        // `\flmHyperref`, `packages::HYPERREF` the LaTeX package `hyperref`
        // that defines `\hyperref`.
        let (command, shapes) = if hooks {
            let hyperref_piece = preamble::HYPERREF;
            lw.require(&Name::from(hyperref_piece));
            (
                preamble::HYPERREF_COMMAND,
                vec![LatexArgShape::Mandatory, LatexArgShape::Mandatory],
            )
        } else {
            let hyperref_package = packages::HYPERREF;
            lw.require(&Name::from(hyperref_package));
            (
                preamble::HYPERREF_PACKAGE_COMMAND,
                vec![LatexArgShape::Optional, LatexArgShape::Mandatory],
            )
        };
        let mut reference = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(command, shapes),
            vec![(String::from(ARG_SLOT), target), (String::from(ARG_SLOT), shown)],
        )?;
        if options.protect {
            reference = protected(cx, lw, reference)?;
        }
        Ok(Lowering::Emit(vec![reference]))
    }
}

/// The two recomposition rules, ready to be registered by the feature: each
/// reads the safe-labels table, the value names, its own render options and
/// those of the backend, produces `flm.pres.latex.macro` and
/// `flm.pres.latex.raw`, and is the default for its own element on the
/// `latex-recomposed` backend only.
pub fn latex_rules() -> Vec<LoweringRule> {
    // Each rule reads both option types: the feature's own for the
    // protection, and the backend's for the hooks.
    let decl = || {
        LoweringDecl::new()
            .reads(SAFE_LABELS)
            .reads(VALUE_SAFE_LABELS)
            .option::<RefsRecomposeOptions>()
            .option::<LatexRecomposedRenderOptions>()
            .produces(MACRO)
            .produces(RAW)
    };
    vec![
        LoweringRule::new(
            REF_LATEX_RULE,
            REF,
            decl(),
            Arc::new(RefLatexRule),
            DefaultMark::OnBackends(vec![BACKEND.into()]),
        )
        .docs(
            "a `\\ref` lowers to the LaTeX hook `\\flmRef` (or to `\\zcref` with the \
             latex-recomposed backend's render option `label_hooks` off) around the safe labels of its targets, its options as an \
             optional argument",
        ),
        LoweringRule::new(
            HYPERREF_LATEX_RULE,
            HYPERREF,
            decl(),
            Arc::new(HyperrefLatexRule),
            DefaultMark::OnBackends(vec![BACKEND.into()]),
        )
        .docs(
            "a `\\hyperref` lowers to the LaTeX hook `\\flmHyperref` (or to `\\hyperref` with the \
             latex-recomposed backend's render option `label_hooks` off) around the safe label of its target and the \
             author's text, recomposed as content",
        ),
    ]
}

/// Registers what the `refs` feature contributes to the lowering of the
/// `latex-recomposed` backend: the peer declarations of the backend and of
/// the two LaTeX constructs the rules stage, then [`latex_rules`]. The
/// safe-labels tables and stages and the options the rules read belong to the
/// feature and are registered beside this.
///
/// The feature registers neither the backend nor the construct elements; it
/// writes a peer declaration ([`PeerName`](crate::core::registry::PeerName))
/// of each, with each construct's payload type as its signature, so that a
/// registry without the LaTeX features still builds, with the rules set aside
/// (excluded from rule selection).
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rules of references are \
         the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX macro-call construct, which the recomposition rules of references and the \
         label helpers stage",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX raw-node construct (text written byte for byte), which the recomposition \
         rules of references and the label helpers stage",
    )?;
    for rule in latex_rules() {
        b.lowering_rule(rule)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::numbering::PluralWords;

    #[test]
    fn a_word_loses_the_tie_to_its_number() {
        assert_eq!(untied("Fig.~"), "Fig.");
        assert_eq!(untied("Figures~"), "Figures");
        assert_eq!(untied("Table"), "Table");
        assert_eq!(untied(" Tab. "), "Tab.");
    }

    #[test]
    fn the_reference_type_keys_take_the_variants_flm_writes_for_them() {
        // The capital variant for `Name-*`, the lowercase one for `name-*`,
        // the base words where either is absent.
        let words = PrefixWords::new("Fig.~", "Figs.~")
            .with_variant(CAPITAL_VARIANT, PluralWords::new("Figure~", "Figures~"));
        assert_eq!(
            reference_type_words(&words),
            [("Name-sg", "Figure"), ("name-sg", "Fig."), ("Name-pl", "Figures"), ("name-pl", "Figs.")]
        );
        let words = PrefixWords::new("Lemma", "Lemmas")
            .with_variant(LOWERCASE_VARIANT, PluralWords::new("lemma", "lemmas"))
            .with_variant(ABBREVIATED_VARIANT, PluralWords::new("Lem.", "Lems."));
        assert_eq!(
            reference_type_words(&words),
            [
                ("Name-sg", "Lemma"),
                ("name-sg", "lemma"),
                ("Name-pl", "Lemmas"),
                ("name-pl", "lemmas"),
                ("Name-sg-ab", "Lem."),
                ("name-sg-ab", "Lem."),
                ("Name-pl-ab", "Lems."),
                ("name-pl-ab", "Lems."),
            ]
        );
    }

    #[test]
    fn the_two_spellings_are_told_apart_by_the_table() {
        let mut table = SafeLabels::default();
        let sec = Label::new("sec", "a");
        table.allocate(REF_DOMAIN, &sec.to_string());
        assert_eq!(
            label_spelling(&table, "sec:a", Some(&sec)),
            LabelSpelling::Safe(String::from("ref1"))
        );
        // A type marked raw is allocated nothing and written as written.
        let raw = Label::new("fig", "b");
        assert_eq!(
            label_spelling(&table, "fig:b", Some(&raw)),
            LabelSpelling::Raw(String::from("fig:b"))
        );
        // A label with no type is allocated a name like any other and
        // written by it; the stage keys it by its text.
        table.allocate(REF_DOMAIN, "plain");
        assert_eq!(
            label_spelling(&table, "plain", None),
            LabelSpelling::Safe(String::from("ref2"))
        );
        assert_eq!(label_spelling(&table, "plain", None).text(), "ref2");
    }

    #[test]
    fn each_rule_is_the_default_for_its_element_on_the_recomposed_backend_only() {
        let rules = latex_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].element, REF);
        assert_eq!(rules[1].element, HYPERREF);
        for rule in &rules {
            assert!(rule.default.is_specific_to(&BACKEND.into()));
            assert_eq!(rule.decl.reads.len(), 2);
            assert_eq!(rule.decl.produces.len(), 2);
            // Each reads both option types — the feature's own for the
            // protection, the backend's for the hooks.
            assert_eq!(rule.decl.options.len(), 2);
        }
    }
}
