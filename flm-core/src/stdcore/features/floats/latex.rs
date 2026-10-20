//! The lowering rule of the [`floats`](super) feature for the
//! `latex-recomposed` backend, which writes a float as the environment
//! `flmFloat`, and the feature's definition piece, which defines that
//! environment.
//!
//! | written | LaTeX |
//! |---|---|
//! | `\begin{figure}\includegraphics{a.png}\caption{A photo}\label{figure:a}\end{figure}` | `\begin{flmFloat}{figure}{NumCap}\flmIncludeGraphics{a.png}\caption{A photo}\flmLabels{\label{ref1}}\end{flmFloat}` |
//! | `\begin{figure}\includegraphics{a.png}\label{figure:b}\end{figure}` | `\begin{flmFloat}{figure}{NumOnly}\flmIncludeGraphics{a.png}\caption{}\flmLabels{\label{ref1}}\end{flmFloat}` |
//! | `\begin{figure}\nonumber\includegraphics{a.png}\caption{A photo}\label{figure:a}\end{figure}` | `\begin{flmFloat}{figure}{CapOnly}\flmIncludeGraphics{a.png}\caption{A photo}\phantomsection\flmLabelsWithDisplayText{A photo}{\flmLabels{\label{ref1}}}\end{flmFloat}` |
//! | `\begin{figure}\includegraphics{a.png}\end{figure}` | `\begin{flmFloat}{figure}{Bare}\flmIncludeGraphics{a.png}\end{flmFloat}` |
//!
//! # The environment and its four cases
//!
//! The rule [`LATEX_RULE`] ([`FloatLatexRule`]) writes a float as
//! `\begin{flmFloat}{<type>}{<case>}`, then the float's content, lowered
//! through the ordinary rules (an image as the `graphics` feature's
//! `\flmIncludeGraphics`), then `\caption{<caption>}` where the float has a
//! caption (the caption's full content), then its labels, then
//! `\end{flmFloat}`. The **case** ([`FloatCase`]) says whether the float is
//! numbered and whether it has a caption: `NumCap`, `NumOnly`, `CapOnly` or
//! `Bare`.
//!
//! Whether the float is numbered is taken from the float type's numbering
//! policy and the float's `\label` and `\nonumber`, as [`read_float`] reads
//! them, and never from the run's counter table: LaTeX numbers the float, so
//! a float that its policy numbers is written as numbered even when the
//! numbering feature is not registered. `\nonumber` is never written, since
//! the case carries it. A `NumOnly` float is written with an empty
//! `\caption{}` before its labels, so that LaTeX shows its number.
//!
//! The labels are written as their safe labels, the keys the recomposed
//! LaTeX writes for FLM labels (`ref1`, `ref2`, …;
//! [`SafeLabels`]), inside the LaTeX hook `\flmLabels`
//! ([`refs::stage_label_group`]). A numbered float's labels follow its
//! caption, so that they refer to its number. A `CapOnly` float has no number
//! for a label to refer to: its labels are written after `\phantomsection`,
//! which places a link target there, inside the LaTeX hook
//! `\flmLabelsWithDisplayText{<shown>}{\flmLabels{…}}`
//! ([`refs::stage_labels_with_display_text`]), so that a reference to the
//! float prints `<shown>`, as on FLM's other outputs. `<shown>` is the
//! caption's alternative text where its optional argument gives one, else its
//! full content, lowered as ordinary content. An xnote in it (a note set apart
//! from the running text, such as a footnote, an endnote or a side note;
//! [`xnotes`](crate::stdcore::features::xnotes)) is written there too,
//! and LaTeX typesets it again at every reference, so an author keeps a
//! footnote out of a reference by giving the caption an alternative text
//! without it. With the
//! render option
//! [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! off, the labels are written as bare `\label` commands, without either
//! LaTeX hook. A `Bare` float's labels are written in `\flmLabels` alone: a
//! reference to it has nothing to show, which FLM's other outputs report.
//!
//! # The definition piece and its LaTeX hooks
//!
//! A definition piece ([`LatexPreambleDef`]) is a unit of LaTeX definitions
//! that a feature registers under an identifier `flm.latex.def.<name>`; a
//! lowering rule requires it by identifier, and the style file holds the text
//! of every piece the document requires. A LaTeX hook is a macro or an
//! environment that the recomposed LaTeX writes and the style file defines
//! with a plain default, which a user redefines in the document's preamble
//! ([LaTeX hooks](crate::stdcore::backends::latex::preamble); [the LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks) in the rendering guide).
//!
//! The feature's definition piece, [`PIECE`] (`flm.latex.def.floats`,
//! [`definition_piece`]), defines the environment `flmFloat` with
//! `\ProvideDocumentEnvironment{flmFloat}{m m}`. The environment calls
//! `\flmFloatSetUseConfig<case>{<type>}{<placement>}` to choose how a float of
//! that case is set, `<placement>` being the value of
//! `\flmFloatPlacementArgs<case>`, then centers the content with
//! `\centering`. By default, a numbered float is set in LaTeX's environment of
//! the type's name at the placement `[tbph]`, which lets LaTeX move it; an
//! unnumbered float is set where it is written, in a `center` environment in
//! which `\caption` works as in a float of the type, and a `CapOnly` float's
//! caption is set without a number, as `\caption*` of the `caption` package,
//! which the piece requests ([`packages::CAPTION`]; [package
//! requests](crate::stdcore::backends::latex::packages)). Each of the eight
//! commands `\flmFloatSetUseConfig<case>` and `\flmFloatPlacementArgs<case>` is
//! a LaTeX hook: redefine it, before or after loading the style file, to set
//! floats differently.
//!
//! # The reference type of each float type
//!
//! The recomposed LaTeX writes a reference through the LaTeX hook `\flmRef`,
//! whose default is `\zcref` of the `zref-clever` package, which finds a
//! target's words by its reference type. For each float type that the
//! document uses, the rule writes a declaration of the type's words as that
//! reference type
//! (`\zcRefTypeSetup{figure}{Name-sg={Figure},name-sg={Fig.},Name-pl={Figures},name-pl={Figs.}}`),
//! mapped as for every reference type of the LaTeX family
//! ([`refs::reference_type_words`]): the variant `capital` where the type's
//! words declare it (the base words otherwise) as the capitalized keys, which
//! `\ref[S]` and `\ref[capital]` print; the variant `lowercase` where
//! declared (the base words otherwise) as the lowercase keys, which a
//! reference prints by default; and the variant `abbreviated`, where
//! declared, as the abbreviated keys. Each word is written without the tie
//! that joins it to a number on FLM's own outputs, since `zref-clever` puts
//! its own space between a name and a number. The rule stages this
//! declaration as a definition construct of kind
//! [`LatexDefinitionKind::Declaration`], which the style file holds once per
//! float type, guarded on the control sequence `\flm@float@reftype@<type>`,
//! which it defines. An author
//! who wants other words calls `\zcRefTypeSetup` after loading the style
//! file. The words of every float type are compiled as FLM content once, when
//! the registry is built; a word that does not compile fails the build.
//!
//! # A float type that a host configures
//!
//! `figure` and `table` are LaTeX's own environments. The environment of a
//! float type that a host configures (`diagram`) is written by the type's
//! name, and the author's LaTeX must define it as a float environment with a
//! counter of the type's name, for instance through the `newfloat` package,
//! as for a configured list kind or quotation environment; the style file
//! declares none.
//!
//! The rule reads the float through [`read_float`], as every rule lowering a
//! float does, so the recomposed LaTeX reports the same problems as the other
//! backends. The rendering guide shows the result ([floats in the recomposed
//! LaTeX](crate::guide::rendering#floats-in-the-recomposed-latex)).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use techy::core::node::{BuildId, NodeRef, NodeTree};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder, Finalizer, LateInit, LateInitError};
use crate::lang::rewrites::userdefs::template::{PlaceholderVocabulary, TemplateLanguage};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, DEFINITION, ENVIRONMENT, MACRO, RAW};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload,
    LatexMacroPayload, LatexPreambleDef, LatexRawPayload, packages, preamble,
};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::{lower_run, lower_written_pieces};
use crate::stdcore::backends::pres::BODY_SLOT;
use crate::stdcore::features::numbering::{COUNTERS, compile_text, instantiate_fixed};
use crate::stdcore::features::refs::{self, ANCHORS, SAFE_LABELS, SafeLabels};

use super::body::{Caption, is_float_field, read_float};
use super::config::FloatType;
use super::content::Handlers;
use super::elements::FloatElement;
use super::{CAPTION_FIELD, CAPTION_TEXT_ARG, CHECK_HANDLERS, FEATURE, FLOAT, TYPES_SLOT};

/// The name of the float's rule on the `latex-recomposed` backend,
/// `flm.floats.float.lower.latex-recomposed`, whose body is
/// [`FloatLatexRule`].
pub const LATEX_RULE: &str = "flm.floats.float.lower.latex-recomposed";

/// The name of the feature's definition piece, `floats`; its identifier is
/// `flm.latex.def.floats` ([`PIECE`]).
pub const PIECE_NAME: &str = "floats";

/// The identifier of the feature's definition piece, `flm.latex.def.floats`
/// ([`definition_piece`]).
pub const PIECE: &str = "flm.latex.def.floats";

/// The name of the environment a float is written as, `flmFloat`, a LaTeX
/// hook that the feature's definition piece defines.
pub const ENVIRONMENT_NAME: &str = "flmFloat";

/// The prefix of the names of the four commands that choose how a float of
/// one case is set, `flmFloatSetUseConfig`: `\flmFloatSetUseConfigNumCap` and
/// the three others, one per [`FloatCase`].
pub const SET_USE_CONFIG_PREFIX: &str = "flmFloatSetUseConfig";

/// The prefix of the names of the four commands that hold the placement of a
/// float of one case, `flmFloatPlacementArgs`:
/// `\flmFloatPlacementArgsNumCap` and the three others, one per
/// [`FloatCase`].
pub const PLACEMENT_ARGS_PREFIX: &str = "flmFloatPlacementArgs";

/// The prefix of the control sequence on which the declaration of a float
/// type's reference type is guarded, `flm@float@reftype@`, followed by the
/// type's name.
pub const REFERENCE_TYPE_GUARD_PREFIX: &str = "flm@float@reftype@";

/// How a float is written on the recomposed LaTeX: whether it is numbered,
/// under its type's numbering policy and its `\label` and `\nonumber`, and
/// whether it has a caption. The case's name is the second argument of the
/// environment `flmFloat`: `\begin{flmFloat}{figure}{NumCap}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatCase {
    /// Numbered, with a caption: `Figure 1: A photo`.
    NumCap,
    /// Numbered, without a caption: `Figure 1`, written with an empty
    /// `\caption{}`.
    NumOnly,
    /// A caption and no number: `A photo`.
    CapOnly,
    /// Neither a number nor a caption.
    Bare,
}

impl FloatCase {
    /// The four cases, in the order in which the definition piece defines
    /// their commands.
    pub const ALL: [FloatCase; 4] =
        [FloatCase::NumCap, FloatCase::NumOnly, FloatCase::CapOnly, FloatCase::Bare];

    /// Returns the case of a float, given whether it is numbered and whether
    /// it has a caption.
    pub fn of(numbered: bool, captioned: bool) -> FloatCase {
        match (numbered, captioned) {
            (true, true) => FloatCase::NumCap,
            (true, false) => FloatCase::NumOnly,
            (false, true) => FloatCase::CapOnly,
            (false, false) => FloatCase::Bare,
        }
    }

    /// Returns the case's name, as the environment's argument and the names
    /// of the case's two commands write it: `NumCap`, `NumOnly`, `CapOnly`,
    /// `Bare`.
    pub fn as_str(self) -> &'static str {
        match self {
            FloatCase::NumCap => "NumCap",
            FloatCase::NumOnly => "NumOnly",
            FloatCase::CapOnly => "CapOnly",
            FloatCase::Bare => "Bare",
        }
    }

    /// Returns whether a float of the case is numbered.
    pub fn is_numbered(self) -> bool {
        matches!(self, FloatCase::NumCap | FloatCase::NumOnly)
    }
}

/// Returns the feature's definition piece, `flm.latex.def.floats`, which
/// defines the environment `flmFloat` and the eight LaTeX hooks it calls, the
/// commands `\flmFloatSetUseConfig<case>` and `\flmFloatPlacementArgs<case>`
/// of the four cases. It requests the `caption` package
/// ([`packages::CAPTION`]) for `\caption*`. See the [module
/// documentation](self#the-definition-piece-and-its-latex-hooks).
pub fn definition_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% Floats: a float of FLM's figure, table or a configured float type is
% written as \\begin{flmFloat}{<type>}{<case>} <content> \\caption{<caption>}
% <labels> \\end{flmFloat}, <case> being NumCap (numbered, with a caption),
% NumOnly (numbered, no caption: an empty \\caption{} shows the number),
% CapOnly (a caption, no number) or Bare (neither).
% \\flmFloatSetUseConfig<case>{<type>}{<placement>}: how a float of the case is
%   set. By default a numbered float is set in the environment <type> at
%   <placement>, and an unnumbered one where it is written, in a center
%   environment in which \\caption works as in the float <type>; a CapOnly
%   float's caption is set with no number (\\caption* of the caption package).
% \\flmFloatPlacementArgs<case>: the placement of a float of the case, [tbph]
%   for the numbered cases and [h] for the others.
% The content is centered. Redefine these commands to set floats differently.
\\providecommand\\flmFloatPlacementArgsNumCap{[tbph]}
\\providecommand\\flmFloatPlacementArgsNumOnly{[tbph]}
\\providecommand\\flmFloatPlacementArgsCapOnly{[h]}
\\providecommand\\flmFloatPlacementArgsBare{[h]}
\\def\\flm@float@infloat#1#2{\\edef\\flm@float@begin{\\noexpand\\begin{#1}#2}\\def\\flm@float@end{\\end{#1}}}
\\def\\flm@float@incenter#1{\\def\\flm@float@begin{\\begin{center}\\def\\@captype{#1}}\\def\\flm@float@end{\\end{center}}}
\\def\\flm@float@starcaption{\\let\\flm@float@caption\\caption\\def\\caption{\\flm@float@caption*}}
\\ProvideDocumentCommand\\flmFloatSetUseConfigNumCap{m m}{\\flm@float@infloat{#1}{#2}}
\\ProvideDocumentCommand\\flmFloatSetUseConfigNumOnly{m m}{\\flm@float@infloat{#1}{#2}}
\\ProvideDocumentCommand\\flmFloatSetUseConfigCapOnly{m m}{\\flm@float@incenter{#1}\\flm@float@starcaption}
\\ProvideDocumentCommand\\flmFloatSetUseConfigBare{m m}{\\flm@float@incenter{#1}}
\\ProvideDocumentEnvironment{flmFloat}{m m}{%
  \\edef\\flm@float@placement{\\csname flmFloatPlacementArgs#2\\endcsname}%
  \\csname flmFloatSetUseConfig#2\\endcsname{#1}{\\flm@float@placement}%
  \\flm@float@begin\\centering}{\\flm@float@end}
",
        "`\\begin{flmFloat}{type}{case}`: the LaTeX hook environment of a float, with \
         `\\flmFloatSetUseConfig<case>` and `\\flmFloatPlacementArgs<case>` for each of its four \
         cases",
    )
    .requires([packages::CAPTION])
}

/// The words of one float type's reference type in `zref-clever`, compiled
/// as FLM content: the capitalized singular and plural, the lowercase
/// singular and plural, and the abbreviated ones where the type's words
/// declare them.
struct ReferenceWords {
    /// The keys `Name-sg`, `name-sg`, `Name-pl`, `name-pl`, then the four
    /// abbreviated keys where declared, each with its word.
    keys: Vec<(&'static str, Arc<NodeTree<Flm>>)>,
}

/// The compiled words of every float type of the registry, by type name,
/// filled once the language is built ([`CompileWords`]).
#[derive(Default)]
pub(super) struct TypeWords {
    by_type: OnceLock<BTreeMap<Name, ReferenceWords>>,
}

impl std::fmt::Debug for TypeWords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&Name> = self.by_type.get().map(|by| by.keys().collect()).unwrap_or_default();
        f.debug_struct("TypeWords").field("by_type", &names).finish()
    }
}

/// Compiles the word `text` of the float type `name` as FLM content without
/// placeholders.
fn compile_word(
    name: &Name,
    text: &str,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<Arc<NodeTree<Flm>>, LateInitError> {
    let what = format!("the words of the float type `{name}`");
    let refused = |detail: String| {
        LateInitError::new(format!(
            "the word `{text}` of the float type `{name}` is not valid FLM text: {detail}"
        ))
    };
    let template = compile_text(text, &what, &PlaceholderVocabulary::new(0), lang, language)
        .map_err(|error| refused(error.to_string()))?;
    instantiate_fixed(&template).map_err(refused)
}

/// Returns the compiled words of `float_type`, under the keys of its
/// reference type ([`refs::reference_type_words`]).
fn reference_words(
    float_type: &FloatType,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<ReferenceWords, LateInitError> {
    let mut keys = Vec::new();
    for (key, text) in refs::reference_type_words(&float_type.words) {
        keys.push((key, compile_word(&float_type.name, text, lang, language)?));
    }
    Ok(ReferenceWords { keys })
}

/// The build hook (a [`Finalizer`]) that hands every float type of the
/// service slot [`TYPES_SLOT`], the feature's own and those other features
/// contribute, to the late-initialization hook that compiles their words once
/// the language is built ([`CompileWords`]).
struct CollectTypes {
    words: Arc<TypeWords>,
}

impl Finalizer for CollectTypes {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let types: Vec<FloatType> = match b.services() {
            Some(services) => services.services_named::<FloatType>(TYPES_SLOT).cloned().collect(),
            None => {
                return Err(BuildError::Feature {
                    feature: b.feature().clone(),
                    detail: String::from("the types slot must be assembled before the finalizer runs"),
                });
            }
        };
        b.late_init(Arc::new(CompileWords { words: Arc::clone(&self.words), types }))
    }
}

/// The build hook (a [`LateInit`]) that compiles the words of every float
/// type as FLM content, once the language is built. A word that does not
/// compile fails the registry build: the words are configuration, so the
/// problem is reported once rather than at every float.
struct CompileWords {
    words: Arc<TypeWords>,
    types: Vec<FloatType>,
}

impl LateInit for CompileWords {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "the template language of the floats feature: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let mut by_type = BTreeMap::new();
        for float_type in &self.types {
            by_type.insert(float_type.name.clone(), reference_words(float_type, lang, &language)?);
        }
        // The hook runs once per registration of the feature, and a registry
        // holds the feature once, so the cell is empty here.
        let _ = self.words.by_type.set(by_type);
        Ok(())
    }
}

/// The body of [`LATEX_RULE`]: it stages the declaration of the float type's
/// reference type, which goes to the style file, then the environment
/// `flmFloat` with the float's type and case as arguments, around its content,
/// its caption and its labels. See the [module documentation](self).
#[derive(Debug)]
pub struct FloatLatexRule {
    handlers: Arc<Handlers>,
    words: Arc<TypeWords>,
}

/// Stages a raw node (`flm.pres.latex.raw`) holding the LaTeX `text`.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Stages a call of the macro `\<name>` with arguments of the shapes
/// `shapes`, the content of each argument given in order in `slots`.
fn command(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    name: &str,
    shapes: Vec<LatexArgShape>,
    slots: Vec<Vec<BuildId>>,
) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(name, shapes),
        slots.into_iter().map(|slot| (String::from(ARG_SLOT), slot)).collect(),
    )
}

/// Lowers what a reference to an unnumbered float with a caption prints, as
/// ordinary content: the caption's alternative text where its optional
/// argument gives one, else its full content, each as the author wrote it.
fn shown(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    caption: &Caption<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(alt) = &caption.alt else {
        return lw.lower_arg_named(cx, &caption.node, CAPTION_TEXT_ARG);
    };
    lower_written_pieces(cx, lw, alt)
}

impl FloatLatexRule {
    /// Stages the declaration of the reference type of `float_type` for the
    /// style file: `\zcRefTypeSetup{<type>}{<keys>}`, guarded on
    /// `\flm@float@reftype@<type>`, which it defines.
    fn reference_type(
        &self,
        float_type: &FloatType,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<BuildId, LowerError> {
        let name = float_type.name.as_str();
        let words = self
            .words
            .by_type
            .get()
            .and_then(|by| by.get(&float_type.name))
            .ok_or_else(|| LowerError::other(format!("the words of the float type `{name}` were not compiled")))?;
        let mut body = refs::stage_reference_type_setup(cx, lw, name, &words.keys)?;
        let guard = format!("{REFERENCE_TYPE_GUARD_PREFIX}{name}");
        body.push(raw(cx, lw, &format!("\\def\\{guard}{{}}"))?);
        let payload =
            LatexDefinitionPayload::new(LatexDefinitionKind::Declaration, &guard, 0).of_feature(FEATURE);
        lw.pres_slots_with(cx, &ElementName::from(DEFINITION), payload, vec![(String::from(BODY_SLOT), body)])
    }
}

impl Lower for FloatLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let reading = read_float(&node, &self.handlers, lw)?;
        let fields = &reading.fields;
        let float_type = &element_value::<FloatElement>(&node)
            .ok_or_else(|| LowerError::structure("a float without its value"))?
            .float_type;
        // The policy's answer, never the counter table's: LaTeX numbers the
        // float. See decision rationale at [§flm-dd-dr:std-floats-latex-first].
        let case = FloatCase::of(fields.numbered, fields.caption.is_some());
        lw.require(&Name::from(PIECE));
        let declaration = self.reference_type(float_type, cx, lw)?;
        // The body as written, the fields left out: on the raw tree this
        // backend reads, the fields stand among the body's children.
        let body: Vec<NodeRef<'_, Flm>> = node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let mut content = Vec::new();
        for run in body.split(is_float_field) {
            content.extend(lower_run(cx, lw, run)?);
        }
        let caption = match &fields.caption {
            Some(caption) => Some(lw.lower_arg_named(cx, &caption.node, CAPTION_TEXT_ARG)?),
            // A numbered float with no caption: an empty one, so that LaTeX
            // shows the number.
            None if case.is_numbered() => Some(Vec::new()),
            None => None,
        };
        if let Some(text) = caption {
            content.push(command(cx, lw, CAPTION_FIELD, vec![LatexArgShape::Mandatory], vec![text])?);
        }
        let mut labels = Vec::new();
        for written in refs::label_uses_in(fields.body.labels.iter().copied()) {
            labels.push(refs::stage_label(cx, lw, &written.text, written.label.as_ref())?);
        }
        match (&fields.caption, case) {
            (Some(caption), FloatCase::CapOnly) if !labels.is_empty() => {
                // No number to refer to: an anchor here, and the labels
                // showing the caption, as an unnumbered heading's do.
                lw.require(&Name::from(packages::HYPERREF));
                content.push(command(cx, lw, preamble::PHANTOMSECTION_COMMAND, Vec::new(), Vec::new())?);
                let hooks = lw.option::<LatexRecomposedRenderOptions>()?.label_hooks;
                let group = refs::stage_label_group(cx, lw, labels, false)?;
                if hooks {
                    let shown = shown(cx, lw, caption)?;
                    content.push(refs::stage_labels_with_display_text(cx, lw, shown, group)?);
                } else {
                    content.extend(group);
                }
            }
            _ => content.extend(refs::stage_label_group(cx, lw, labels, false)?),
        }
        let type_arg = vec![raw(cx, lw, float_type.name.as_str())?];
        let case_arg = vec![raw(cx, lw, case.as_str())?];
        let environment = lw.pres_slots_with(
            cx,
            &ElementName::from(ENVIRONMENT),
            LatexEnvironmentPayload::new(ENVIRONMENT_NAME, [LatexArgShape::Mandatory, LatexArgShape::Mandatory]),
            vec![
                (String::from(ARG_SLOT), type_arg),
                (String::from(ARG_SLOT), case_arg),
                (String::from(BODY_SLOT), content),
            ],
        )?;
        Ok(Lowering::Emit(vec![declaration, environment]))
    }
}

/// Returns the float's rule on the `latex-recomposed` backend,
/// [`LATEX_RULE`], the default on that backend only. It tests a float's
/// content against the content handlers `handlers` and writes the float
/// types' compiled words `words`.
fn latex_rule(handlers: Arc<Handlers>, words: Arc<TypeWords>) -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        FLOAT,
        LoweringDecl::new()
            .reads_opt(ANCHORS)
            .reads_opt(COUNTERS)
            .reads_opt(SAFE_LABELS)
            .requires(CHECK_HANDLERS)
            .option::<LatexRecomposedRenderOptions>()
            .produces(DEFINITION)
            .produces(ENVIRONMENT)
            .produces(MACRO)
            .produces(RAW),
        Arc::new(FloatLatexRule { handlers, words }),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a float lowers to the LaTeX hook environment `flmFloat` over its type and case, \
         around its content, `\\caption{…}` and its labels, after the declaration of its type's \
         reference type",
    )
}

/// Registers what the feature contributes to the `latex-recomposed` backend:
/// the peer declarations of the backend, of the LaTeX constructs the rule
/// stages and of the safe-labels table it reads; the definition piece, as a
/// peer contribution to the service slot of definition pieces
/// ([`preamble::SLOT`]); the build hooks that compile the float types' words;
/// and [`latex_rule`]. A peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) tells the registry build
/// that another feature owns a name. A registry without the LaTeX features
/// therefore builds, with the rule set aside (excluded from rule selection;
/// see [`PeerName`](crate::core::registry::PeerName)).
pub(super) fn register(b: &mut FeatureBuilder<'_>, handlers: Arc<Handlers>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rule is the default",
    )?;
    b.peer_element::<LatexDefinitionPayload>(
        DEFINITION,
        "the LaTeX family's definition construct, which the recomposition rule stages for the \
         declaration of a float type's reference type",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        ENVIRONMENT,
        "the LaTeX family's environment construct, which the recomposition rule stages for the \
         float",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages for the \
         caption and the labels",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, in which the environment's arguments, the \
         reference type's keys and a label are written",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the recomposition rule reads for the \
         safe label of each label",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    let words = Arc::new(TypeWords::default());
    b.finalizer(Arc::new(CollectTypes { words: Arc::clone(&words) }))?;
    b.lowering_rule(latex_rule(handlers, words))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_is_named_as_its_identifier_says_and_requests_the_caption_package() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), PIECE);
        assert_eq!(piece.requires, [Name::from(packages::CAPTION)]);
    }

    #[test]
    fn the_piece_defines_the_environment_and_the_two_commands_of_each_case() {
        let piece = definition_piece();
        for case in FloatCase::ALL {
            for prefix in [SET_USE_CONFIG_PREFIX, PLACEMENT_ARGS_PREFIX] {
                let name = format!("\\{prefix}{}{{", case.as_str());
                assert!(piece.content.contains(&name), "{name}");
            }
        }
        assert!(piece.content.contains("\\ProvideDocumentEnvironment{flmFloat}{m m}"));
        assert!(piece.content.contains("\\providecommand\\flmFloatPlacementArgsNumCap{[tbph]}"));
        assert!(piece.content.contains("\\providecommand\\flmFloatPlacementArgsCapOnly{[h]}"));
        assert!(piece.content.contains("\\flm@float@caption*"));
        assert!(piece.content.contains("\\centering"));
    }

    #[test]
    fn the_case_follows_the_number_and_the_caption() {
        assert_eq!(FloatCase::of(true, true).as_str(), "NumCap");
        assert_eq!(FloatCase::of(true, false).as_str(), "NumOnly");
        assert_eq!(FloatCase::of(false, true).as_str(), "CapOnly");
        assert_eq!(FloatCase::of(false, false).as_str(), "Bare");
        assert!(FloatCase::NumOnly.is_numbered() && !FloatCase::CapOnly.is_numbered());
    }
}
