//! The lowering rules of the [`theorems`](super) feature for the
//! `latex-recomposed` backend, and the definition pieces they require. The
//! rendering guide describes this output for users under [Theorems in the
//! recomposed LaTeX](crate::guide::rendering#theorems-in-the-recomposed-latex).
//!
//! The theorem's rule ([`TheoremLatexRule`], [`THEOREM_LATEX_RULE`]) writes a
//! theorem back as the environment the author wrote, and writes into the
//! style file the declarations that define that environment for LaTeX. LaTeX
//! then sets the theorem's heading (the kind's word, the number and the
//! title) and a proof's closing mark; the class's heading template is not
//! used on this backend. A proof's back-reference is written in one of two
//! forms, which the backend's render option
//! [`resolved_proof_refs`](LatexRecomposedRenderOptions::resolved_proof_refs)
//! chooses:
//!
//! - the **authored form** (the default): the back-reference is written as
//!   the author wrote it, for the `phfthm` LaTeX package to read;
//! - the **resolved form**: the back-reference is written as the proof's
//!   title, a reference to the statement, and the recomposed LaTeX needs the
//!   `amsthm` package alone.
//!
//! | written | LaTeX, the authored form (the default) | LaTeX, the resolved form |
//! |---|---|---|
//! | `\begin{theorem}[My Theorem]\label{thm:a}…\end{theorem}` | `\begin{theorem}[My Theorem]\flmLabels{\label{ref1}}…\end{theorem}` | the same |
//! | `\begin{proof}[Sketch]…\end{proof}` | `\begin{proof}[{\flmProofTitle{Sketch}}]…\end{proof}` | the same |
//! | `\begin{proof}[*thm:a]…\end{proof}` | `\begin{proof}[*ref1]…\end{proof}` | `\begin{proof}[{\flmProofTitle{\flmRef{ref1}}}]…\end{proof}` |
//! | `\begin{proof}[**thm:a]…\end{proof}` | `\begin{proof}[**ref1]…\end{proof}` | `\begin{proof}…\end{proof}` |
//! | `\begin{lemma}\noproofref…\end{lemma}` | `\begin{lemma}\noproofref…\end{lemma}` | the same |
//!
//! The rules use the LaTeX family's vocabulary: a **LaTeX hook** is a macro
//! that the recomposed LaTeX writes and the style file defines with a plain
//! default, which a user redefines in the document's preamble
//! ([`preamble`]; the rendering
//! guide lists the hooks under [the LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks)); a **definition
//! piece** is a unit of LaTeX definitions that a rule requires by identifier
//! and the style file holds ([`LatexPreambleDef`]); a **package request** is
//! a LaTeX package that a rule or a definition piece requires by identifier
//! ([`packages`]); and a **safe
//! label** is the key the recomposed LaTeX writes for an FLM label (`ref1`,
//! `ref2`, …; [`SafeLabels`]).
//!
//! # A theorem
//!
//! [`TheoremLatexRule`] writes a theorem as the environment of its kind's
//! name. Its title, where it has one, is the environment's optional argument,
//! lowered through the ordinary rules; the backend writes it as `[My
//! Theorem]`, or as `[{…}]` as soon as it holds a command. Right after the
//! begin line come the theorem's labels, each under its safe label, inside
//! the labels hook `\flmLabels`, as every group of labels is written
//! ([`stage_label`](crate::stdcore::features::refs::stage_label),
//! [`stage_label_group`](crate::stdcore::features::refs::stage_label_group));
//! then `\noproofref` where the author wrote it; then the body.
//!
//! **A proof's title** (the title of a kind whose class reads
//! back-references, [`relation_ref`](super::TheoremClass::relation_ref)) is
//! written inside the proof title hook, `[{\flmProofTitle{Sketch}}]`
//! ([`PROOF_TITLE_COMMAND`]). The definition piece of the chosen form
//! defines the hook with the default that the form's package needs: in the
//! authored form the title alone, which `phfthm` sets as "Proof of Sketch"
//! ([`authored_piece`]); in the resolved form "Proof (Sketch)", which
//! `amsthm` sets as the whole heading, as FLM's other outputs show a proof's
//! title ([`resolved_piece`]). An author redefines the hook to show a proof's
//! title otherwise. A title that begins with a star on a proof but is not a
//! well-formed back-reference is written in the hook as every title is; the
//! feature's check reports it.
//!
//! **`\noproofref`** says that no proof of the statement is coming. It is
//! written as the author wrote it in both forms, because LaTeX has a use for
//! it: a statement's environment can print where its proof is ("proof on page
//! 12"), and needs to know which statements have none. `phfthm` defines the
//! command; the feature's definition piece ([`definition_piece`]) defines it
//! as a command that does nothing, with `\ProvideDocumentCommand`, so that
//! the definition of a package loaded first is kept.
//!
//! # A proof's back-reference
//!
//! The statement a proof refers to (`[*thm:a]`, `[**thm:a]`) is written under
//! the safe label of its label, read through
//! [`target_spelling`], the
//! function through which every reference site's target is read. A proof
//! inside a synthesized value that a rule splices at lowering (the use site of
//! a kept definition: a user definition whose use sites a plan leaves in the
//! tree; see [kept definitions](crate::stdcore::features::userdefs::kept)) is
//! therefore written under the value names of that value (`ref-v1`, …;
//! [`SafeLabels`]). A back-reference whose label a template's placeholder
//! completes cannot be written in the template of a kept definition:
//! the rule returns [`LowerError::PlaceholderCharacters`], and each use site
//! of the definition is expanded instead.
//!
//! - **The authored form** (render option off, the default). The rule writes
//!   the back-reference as the author wrote it, `[*ref1]` or `[**ref1]`, and
//!   the authored form's definition piece ([`authored_piece`]) requests the `phfthm`
//!   package ([`packages::PHFTHM`]). The package reads that syntax in a
//!   proof's optional argument: `[*ref1]` sets the heading "Proof of
//!   Theorem 1" where the document loads `hyperref` and "Proof of 1" where it
//!   does not (the package refers to the statement through `hyperref`'s
//!   `\autoref`, or through `\ref`), and `[**ref1]` sets "Proof". The rule
//!   does not request `hyperref`: whether a document loads it is its author's
//!   decision. The package reads any other optional argument as the thing
//!   the proof proves and writes "Proof of" before it.
//! - **The resolved form** (render option on). The resolved form's
//!   definition piece ([`resolved_piece`]) requests `amsthm` alone ([`packages::AMSTHM`]). A
//!   back-reference that the proof shows is written as a title is, inside the
//!   proof title hook, the title being the reference hook `\flmRef` around
//!   the statement's safe label: `[{\flmProofTitle{\flmRef{ref1}}}]`, which
//!   sets "Proof (theorem 1)" by default. With the render option
//!   [`label_hooks`](LatexRecomposedRenderOptions::label_hooks) off, the
//!   reference is `\zcref{ref1}`, the command the hook runs by default. A
//!   back-reference that is recorded and not shown writes no optional
//!   argument, and the proof's heading is LaTeX's own, "Proof".
//!
//! # The declarations in the style file
//!
//! LaTeX must know every theorem kind the document uses, so the rule writes
//! a declaration of the kind into the style file, in the block of the
//! definitions a feature writes from its configuration
//! ([`style`](crate::stdcore::backends::latex::style)). The kind's display
//! name is its base singular word (`Lemma`), which is FLM text, so the
//! declaration is lowered through the rules at the theorem. It is staged
//! (built into the lowered tree; see [`Lowerer`]) as the LaTeX family's
//! definition construct with the kind
//! [`LatexDefinitionKind::Declaration`], which the style file writes once per
//! name, guarded on the name it declares: `\ifcsname lemma\endcsname\else …
//! \fi`. A kind that the document class, a package or the author's own
//! preamble defines before the style file is loaded is therefore kept, and
//! the declaration does nothing; an author with a setup of their own defines
//! their kinds first. The style file declares only the kinds the document
//! uses.
//!
//! | the declaration | the authored form | the resolved form |
//! |---|---|---|
//! | the theorem style of a numbered kind's class | `\newtheoremstyle{theoremlike}{}{}{\itshape}{}{\bfseries}{.}{5pt plus 1pt minus 1pt}{}` | the same |
//! | the shared counter, before the first kind counting in it | none: `phfthm`'s own counter `phfthmcounter` | `\newcounter{flmtheorem}`, guarded on `c@flmtheorem` |
//! | a numbered kind counting in the shared counter | `\phfMakeTheorem[thmstyle=theoremlike,counter=phfthmcounter]{lemma}{Lemma}` | `\begingroup\theoremstyle{theoremlike}\newtheorem{lemma}[flmtheorem]{Lemma}\endgroup` |
//! | a numbered kind with a counter of its own, expecting no proof | `\phfMakeTheorem[thmstyle=exerciselike,proofref=false]{exercise}{Exercise}` | `\begingroup\theoremstyle{exerciselike}\newtheorem{exercise}{Exercise}\endgroup` |
//! | an unnumbered kind | `\phfMakeProofEnv[defaultproofname={Proof}]{proof}` | `\newenvironment{proof}[1][{Proof}]{\begin{proof}[{#1}]}{\end{proof}}` |
//!
//! - **Theorem styles.** A numbered kind is set in the `amsthm` theorem style
//!   named after its class. Before the first kind of a class, the style file
//!   declares that style with the parameters of `amsthm`'s `plain` style (a
//!   bold heading and an italic body), guarded on the control sequence
//!   `amsthm` creates for a style (`\th@theoremlike`). A style of that name
//!   declared first is kept: the feature's definition piece
//!   ([`definition_piece`]) declares the shipped class `definitionlike` in
//!   `amsthm`'s `definition` style (an upright body), and an author sets a
//!   class otherwise by declaring a style of the class's name before loading
//!   the style file.
//! - **What `phfthm` provides, in the authored form.** `\phfMakeTheorem`
//!   declares a numbered theorem environment in a given theorem style
//!   (`thmstyle`) and counter (`counter`); with `proofref=false`, which the
//!   rule writes for a kind that expects no proof, the package looks for no
//!   proof of its statements. `\phfMakeProofEnv` declares a proof environment
//!   with a default heading word (`defaultproofname`). The kinds that share
//!   the counter count in the package's own counter, `phfthmcounter`
//!   ([`PHFTHM_SHARED_COUNTER`]). The style file requests the package with
//!   the option `thmset=empty` ([`packages::PHFTHM`]), so that the package
//!   declares no environment of its own and the configuration is the only
//!   source of the kinds declared; an author who loads the package before the
//!   style file gives it the same option.
//! - **What the resolved form declares.** The shared counter is a LaTeX
//!   counter of the feature's own, `flmtheorem` ([`SHARED_LATEX_COUNTER`]),
//!   declared once, guarded on its register, before the first kind counting
//!   in it; every such kind is declared over it with `amsthm`'s
//!   `\newtheorem`. A kind with a counter of its own counts in the counter of
//!   its own name. An unnumbered kind is `amsthm`'s `proof`, which the package
//!   defines, or an environment over it.
//! - **Reference types.** Every numbered kind's declaration also makes the
//!   kind a reference type of its own for the `zref-clever` package, with the
//!   kind's words, and maps the counter its environment steps to that type,
//!   so that a reference to a lemma
//!   counting in the shared counter prints "lemma".
//!
//! # Limitation: theorems numbered within sections
//!
//! The declarations do not yet write the counter's hierarchy: a document that
//! numbers theorems within sections still gets, from LaTeX, one sequence of
//! numbers for the kinds, regardless of the document's numbering options.
//! Writing the hierarchy into the declarations is designed and not yet
//! implemented.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{
    ARG_SLOT, DEFINITION, ENVIRONMENT as ENVIRONMENT_CONSTRUCT, MACRO, RAW,
};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload,
    LatexMacroPayload, LatexPreambleDef, LatexRawPayload, packages, preamble,
};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::lower_arg_named_or_nothing;
use crate::stdcore::backends::pres::BODY_SLOT;
use crate::stdcore::features::refs::{
    self, SAFE_LABELS, SafeLabels, VALUE_SAFE_LABELS, ValueSafeLabels, ZCREF_COMMAND,
    stage_spelling, target_spelling,
};

use super::FEATURE;
use super::elements::{THEOREM, TITLE_ARG, TheoremElement, TitleReading, has_noproofref, title_reading};
use super::heading::{Headings, NAME_ELEMENT, RELATION, TITLE_POINTER};

/// The name of the theorem's rule on the `latex-recomposed` backend,
/// `flm.theorems.theorem.lower.latex-recomposed` ([`TheoremLatexRule`]).
pub const THEOREM_LATEX_RULE: &str = "flm.theorems.theorem.lower.latex-recomposed";

/// The name of the rule of a heading's name node ([`NAME_ELEMENT`]) on the
/// `latex-recomposed` backend, `flm.theorems.name.lower.latex-recomposed`:
/// it lowers the node's own body, the kind's word. The recomposed LaTeX
/// instantiates no heading template, because LaTeX writes a theorem's
/// heading, so this node, like the title pointer and the back-reference,
/// never reaches the backend. The three rules exist because a plan checks
/// that its backend can lower every element a document may hold.
pub const NAME_LATEX_RULE: &str = "flm.theorems.name.lower.latex-recomposed";

/// The name of the rule of a heading's title pointer ([`TITLE_POINTER`]) on
/// the `latex-recomposed` backend,
/// `flm.theorems.title-pointer.lower.latex-recomposed`: it lowers the node's
/// own body; the node never reaches this backend (see [`NAME_LATEX_RULE`]).
pub const TITLE_POINTER_LATEX_RULE: &str = "flm.theorems.title-pointer.lower.latex-recomposed";

/// The name of the rule of a heading's back-reference node ([`RELATION`]) on
/// the `latex-recomposed` backend,
/// `flm.theorems.relation.lower.latex-recomposed`: it lowers the node's own
/// body; the node never reaches this backend (see [`NAME_LATEX_RULE`]).
pub const RELATION_LATEX_RULE: &str = "flm.theorems.relation.lower.latex-recomposed";

/// The command that marks a statement as having no proof, `noproofref`:
/// written as the author wrote it, and defined by the feature's definition
/// piece ([`definition_piece`]) where no package defined it first.
pub const NOPROOFREF_COMMAND: &str = "noproofref";

/// The proof title hook, `flmProofTitle`, a LaTeX hook: a proof's title (a
/// title the author wrote, or, in the resolved form, the reference hook
/// around the statement the proof refers back to) is written as its argument
/// inside the proof environment's optional argument,
/// `\begin{proof}[{\flmProofTitle{Sketch}}]`. The definition piece of the
/// form that the render option chooses defines it, with that form's default
/// ([`authored_piece`], [`resolved_piece`]).
pub const PROOF_TITLE_COMMAND: &str = "flmProofTitle";

/// The name of the feature's definition piece, `theorems`; its identifier is
/// `flm.latex.def.theorems` ([`PIECE`]).
pub const PIECE_NAME: &str = "theorems";

/// The identifier of the feature's definition piece, `flm.latex.def.theorems`
/// ([`definition_piece`]).
pub const PIECE: &str = "flm.latex.def.theorems";

/// The name of the authored form's definition piece, `theorems.authored`; its
/// identifier is `flm.latex.def.theorems.authored` ([`AUTHORED_PIECE`]).
pub const AUTHORED_PIECE_NAME: &str = "theorems.authored";

/// The identifier of the authored form's definition piece,
/// `flm.latex.def.theorems.authored` ([`authored_piece`]).
pub const AUTHORED_PIECE: &str = "flm.latex.def.theorems.authored";

/// The name of the resolved form's definition piece, `theorems.resolved`; its
/// identifier is `flm.latex.def.theorems.resolved` ([`RESOLVED_PIECE`]).
pub const RESOLVED_PIECE_NAME: &str = "theorems.resolved";

/// The identifier of the resolved form's definition piece,
/// `flm.latex.def.theorems.resolved` ([`resolved_piece`]).
pub const RESOLVED_PIECE: &str = "flm.latex.def.theorems.resolved";

/// The counter that the `phfthm` package defines when it is loaded, and in
/// which it numbers its own theorem environments, `phfthmcounter`. In the
/// authored form, every kind of a class that counts in the shared counter is
/// declared over it.
pub const PHFTHM_SHARED_COUNTER: &str = "phfthmcounter";

/// The LaTeX counter that holds the feature's shared counter in the resolved
/// form, `flmtheorem`. The style file declares it once, guarded on its
/// register (`\ifcsname c@flmtheorem\endcsname\else\newcounter{flmtheorem}\fi`),
/// and declares every kind of a class that counts in the shared counter over
/// it, `\newtheorem{lemma}[flmtheorem]{Lemma}`. Its name differs from every
/// kind's, so that it is never the counter of a kind with a counter of its
/// own (which counts in the counter of its own name), and so that an author
/// can number theorems within another counter by giving its name
/// (`\numberwithin{flmtheorem}{section}`).
pub const SHARED_LATEX_COUNTER: &str = "flmtheorem";

/// The feature's definition piece, `flm.latex.def.theorems` ([`PIECE`]),
/// required at every theorem in both forms, beside the definition piece of
/// the chosen form ([`authored_piece`], [`resolved_piece`]). It defines:
///
/// - `\noproofref`, which says that no proof of a statement is coming, as a
///   command that does nothing, with `\ProvideDocumentCommand`, so that where
///   a package defines it first (the `phfthm` package among them), the
///   package's definition is kept. Redefine it, before loading the style file
///   or after it, to act on statements that have no proof.
/// - The theorem style of the shipped class `definitionlike`, declared as
///   `amsthm`'s `definition` style (an upright body), guarded on the control
///   sequence `amsthm` creates for it, `\th@definitionlike`. Every class's
///   kinds are set in the theorem style named after the class, and a class
///   whose style nobody declared gets the `plain` style (see [the declarations
///   in the style file](self#the-declarations-in-the-style-file)); this
///   definition piece, which the style file writes before those declarations, gives the
///   shipped class its own style. Declare a style of the class's name before
///   the style file is loaded to set its kinds your own way.
///
/// The definition piece uses `amsthm`'s `\newtheoremstyle`; the package that each form
/// requests loads `amsthm`.
pub fn definition_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% Theorems. \\noproofref, written after a statement's labels, says that no proof
% of the statement is coming; defined here as nothing where no package (phfthm)
% defined it first.
\\ProvideDocumentCommand\\noproofref{}{}
% The kinds of a class are set in the theorem style of the class's name. The
% shipped class definitionlike is set in amsthm's definition style (upright);
% declare a theorem style of that name before loading this file to change it.
\\ifcsname th@definitionlike\\endcsname\\else\\newtheoremstyle{definitionlike}{}{}{\\normalfont}{}{\\bfseries}{.}{5pt plus 1pt minus 1pt}{}\\fi
",
        "`\\noproofref`: a statement with no proof coming, defined as nothing where no package \
         defined it; the theorem style of the shipped class `definitionlike`",
    )
}

/// The authored form's definition piece, `flm.latex.def.theorems.authored`
/// ([`AUTHORED_PIECE`]), required at every theorem when the render option
/// [`resolved_proof_refs`](LatexRecomposedRenderOptions::resolved_proof_refs)
/// is off. It requests the `phfthm` package ([`packages::PHFTHM`]).
///
/// It defines the proof title hook `\flmProofTitle{<title>}`
/// ([`PROOF_TITLE_COMMAND`]), what a proof's heading shows for a title the
/// author wrote, as the title alone, `#1`: `phfthm` reads a proof's optional
/// argument as the thing the proof proves and writes the rest of the heading
/// itself, so that `\begin{proof}[{\flmProofTitle{Sketch}}]` sets "Proof of
/// Sketch". The hook is defined with `\ProvideDocumentCommand`, so that a
/// definition made before the style file is loaded is kept; redefine it,
/// before loading the style file or after it, to show a proof's title your
/// own way. A document that lists `phfthm` in the render option
/// [`skip_packages`](LatexRecomposedRenderOptions::skip_packages) keeps this
/// default, `#1`: the author who provides the proof environment also
/// provides the heading's wording, and redefines the hook where that
/// environment reads its optional argument otherwise. The resolved form's
/// definition piece ([`resolved_piece`]) defines the same hook with the default its
/// package needs; a style file holds one of the two.
pub fn authored_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        AUTHORED_PIECE_NAME,
        "\
% \\flmProofTitle{<title>}: what a proof's heading sets for a title the document
% wrote, which is written as its argument inside the proof environment's
% optional argument. phfthm reads that argument as the thing the proof proves
% and words the heading itself (Proof of <title>), so by default the title
% alone. Redefine \\flmProofTitle to set a proof's title differently.
\\ProvideDocumentCommand\\flmProofTitle{m}{#1}
",
        "`\\flmProofTitle`, the LaTeX hook of a proof's title, setting the title alone, which \
         `phfthm` words as the thing the proof proves",
    )
    .requires([packages::PHFTHM])
}

/// The resolved form's definition piece, `flm.latex.def.theorems.resolved`
/// ([`RESOLVED_PIECE`]), required at every theorem when the render option
/// [`resolved_proof_refs`](LatexRecomposedRenderOptions::resolved_proof_refs)
/// is on. It requests the `amsthm` package alone ([`packages::AMSTHM`]).
///
/// It defines the proof title hook `\flmProofTitle{<title>}`
/// ([`PROOF_TITLE_COMMAND`]), what a proof's heading shows for a title (one
/// the author wrote, or the reference hook around the statement the proof
/// refers back to), as `\proofname\space (#1)`: `amsthm` sets a proof's
/// optional argument as the whole heading, so that
/// `\begin{proof}[{\flmProofTitle{Sketch}}]` sets "Proof (Sketch)", as FLM's
/// other outputs show a proof's title; `\proofname` is the word `amsthm`
/// defines. The hook is defined with `\ProvideDocumentCommand`, so that a
/// definition made before the style file is loaded is kept; redefine it,
/// before loading the style file or after it, to show a proof's title your
/// own way (`\renewcommand\flmProofTitle[1]{#1}` shows the title alone). The
/// authored form's definition piece ([`authored_piece`]) defines the same hook with the
/// default its package needs; a style file holds one of the two.
pub fn resolved_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        RESOLVED_PIECE_NAME,
        "\
% \\flmProofTitle{<title>}: what a proof's heading sets for a title -- one the
% document wrote, or the statement the proof refers back to -- which is written
% as its argument inside the proof environment's optional argument, the whole
% heading amsthm sets. By default Proof (<title>), \\proofname being the word
% amsthm defines. Redefine \\flmProofTitle to set a proof's title differently;
% \\renewcommand\\flmProofTitle[1]{#1} sets the title alone.
\\ProvideDocumentCommand\\flmProofTitle{m}{\\proofname\\space (#1)}
",
        "`\\flmProofTitle`, the LaTeX hook of a proof's title, setting `Proof (<title>)` as the \
         whole heading `amsthm` sets",
    )
    .requires([packages::AMSTHM])
}

/// Stages the reference hook around the safe label `key`, `\flmRef{<key>}`,
/// or, with the render option
/// [`label_hooks`](LatexRecomposedRenderOptions::label_hooks) off, the
/// command that the hook runs by default, `\zcref{<key>}`; and requires what
/// defines it, as the references feature does for a `\ref`.
fn reference_hook(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    key: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let command = if lw.option::<LatexRecomposedRenderOptions>()?.label_hooks {
        lw.require(&Name::from(preamble::REFS));
        preamble::REF_COMMAND
    } else {
        lw.require(&Name::from(packages::ZREF_CLEVER));
        ZCREF_COMMAND
    };
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(command, [LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), key)],
    )
}

/// Stages the proof title hook around `title`, `\flmProofTitle{<title>}`.
fn proof_title(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    title: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    command(cx, lw, PROOF_TITLE_COMMAND, vec![LatexArgShape::Mandatory], vec![title])
}

/// The lowering rule [`THEOREM_LATEX_RULE`], the default on the
/// `latex-recomposed` backend: the environment of the kind's name around the
/// title, the labels, `\noproofref` and the body, preceded by the
/// declarations the style file needs for the kind. See the [module
/// documentation](self).
#[derive(Debug)]
pub struct TheoremLatexRule {
    headings: Arc<Headings>,
}

impl TheoremLatexRule {
    /// The optional argument of the theorem `node` of the kind `value`: the
    /// title lowered through the rules, a back-reference in the form
    /// `resolved` chooses, or `None` where no title is written.
    fn title(
        &self,
        node: &NodeRef<'_, Flm>,
        value: &TheoremElement,
        resolved: bool,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Option<Vec<BuildId>>, LowerError> {
        match title_reading(node).unwrap_or(TitleReading::Absent) {
            TitleReading::Absent => Ok(None),
            // A malformed back-reference is reported by the `check` workflow
            // and written as the author wrote it: a title, as every output
            // shows it.
            TitleReading::Title | TitleReading::Malformed => {
                let title = lower_arg_named_or_nothing(cx, lw, node, TITLE_ARG)?;
                if value.class.relation_ref {
                    Ok(Some(vec![proof_title(cx, lw, title)?]))
                } else {
                    Ok(Some(title))
                }
            }
            // A kept template's title whose characters a call supplies: the
            // use sites are expanded, and each is lowered with its own.
            TitleReading::Unknown => Err(LowerError::PlaceholderCharacters),
            TitleReading::Relation { text, label, shown } if !resolved => {
                let stars = if shown { "*" } else { "**" };
                let mut title = vec![raw(cx, lw, stars)?];
                let spelling = target_spelling(lw, &text, Some(&label))?;
                title.extend(stage_spelling(cx, lw, &spelling)?);
                Ok(Some(title))
            }
            // The resolved form: the reference hook inside the proof title
            // hook where the back-reference is shown; LaTeX's own heading
            // where it is recorded only.
            TitleReading::Relation { text, label, shown } => {
                if !shown {
                    return Ok(None);
                }
                let spelling = target_spelling(lw, &text, Some(&label))?;
                let key = stage_spelling(cx, lw, &spelling)?;
                let reference = reference_hook(cx, lw, key)?;
                Ok(Some(vec![proof_title(cx, lw, vec![reference])?]))
            }
        }
    }
}

impl Lower for TheoremLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<TheoremElement>(&node)
            .ok_or_else(|| LowerError::structure("a theorem without its element's data"))?;
        let resolved = lw.option::<LatexRecomposedRenderOptions>()?.resolved_proof_refs;
        lw.require(&Name::from(PIECE));
        lw.require(&Name::from(if resolved { RESOLVED_PIECE } else { AUTHORED_PIECE }));
        let declarations = self.declarations(value, resolved, cx, lw)?;
        let mut shapes = Vec::new();
        let mut slots = Vec::new();
        if let Some(title) = self.title(&node, value, resolved, cx, lw)? {
            shapes.push(LatexArgShape::Optional);
            slots.push((String::from(ARG_SLOT), title));
        }
        let mut labels = Vec::new();
        for written in refs::label_uses(&node) {
            labels.push(refs::stage_label(cx, lw, &written.text, written.label.as_ref())?);
        }
        let mut body = refs::stage_label_group(cx, lw, labels, false)?;
        if has_noproofref(&node) {
            body.push(lw.pres_slots_with(
                cx,
                &ElementName::from(MACRO),
                LatexMacroPayload::new(NOPROOFREF_COMMAND, []),
                Vec::new(),
            )?);
        }
        body.extend(lw.lower_body(cx, &node)?);
        slots.push((String::from(BODY_SLOT), body));
        let environment = lw.pres_slots_with(
            cx,
            &ElementName::from(ENVIRONMENT_CONSTRUCT),
            LatexEnvironmentPayload::new(value.name.as_str(), shapes),
            slots,
        )?;
        let mut staged = declarations;
        staged.push(environment);
        Ok(Lowering::Emit(staged))
    }
}

/// The parameters of `amsthm`'s `\newtheoremstyle` after the style's name
/// that give the `plain` style — a bold heading followed by a full stop, an
/// italic body, `amsthm`'s own spacing — which a class's theorem style is
/// declared with where nothing declared it first.
const PLAIN_STYLE_PARAMETERS: &str = "{}{}{\\itshape}{}{\\bfseries}{.}{5pt plus 1pt minus 1pt}{}";

/// Stages a raw node (`flm.pres.latex.raw`) holding the LaTeX text `text`,
/// written byte for byte.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Stages `\<command>` with arguments of the shapes `shapes`, each slot's
/// content given in order.
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

/// Stages a declaration of this feature for the style file, guarded on the
/// control sequence `guard`, whose text is `body`.
fn declaration(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    guard: &str,
    body: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let payload =
        LatexDefinitionPayload::new(LatexDefinitionKind::Declaration, guard, 0).of_feature(FEATURE);
    lw.pres_slots_with(
        cx,
        &ElementName::from(DEFINITION),
        payload,
        vec![(String::from(BODY_SLOT), body)],
    )
}

impl TheoremLatexRule {
    /// The declarations that the theorem of the kind `value` needs in the
    /// style file, in the form `resolved` chooses: its kind's, preceded, in
    /// the resolved form and for a kind that counts in the shared counter, by
    /// the declaration of the LaTeX counter it is declared over. Each is
    /// guarded on its name, and the style file holds each once.
    fn declarations(
        &self,
        value: &TheoremElement,
        resolved: bool,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Vec<BuildId>, LowerError> {
        let mut staged = Vec::new();
        let shared = value.counter.is_some() && value.class.shared_counter;
        if resolved && shared {
            // The shared counter's LaTeX counter, declared once, guarded on
            // its register, before the first kind counting in it.
            let body = vec![raw(cx, lw, &format!("\\newcounter{{{SHARED_LATEX_COUNTER}}}"))?];
            staged.push(declaration(cx, lw, &format!("c@{SHARED_LATEX_COUNTER}"), body)?);
        }
        staged.extend(self.kind_declarations(value, resolved, cx, lw)?);
        Ok(staged)
    }

    /// The declarations of the kind `value` itself: the theorem style of its
    /// class where it is numbered, then its own declaration.
    fn kind_declarations(
        &self,
        value: &TheoremElement,
        resolved: bool,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Vec<BuildId>, LowerError> {
        let mut staged = Vec::new();
        if value.counter.is_some() {
            // The theorem style named after the class, guarded on the
            // control sequence `amsthm` makes for it.
            let class = value.class_name.as_str();
            let body = vec![raw(
                cx,
                lw,
                &format!("\\newtheoremstyle{{{class}}}{PLAIN_STYLE_PARAMETERS}"),
            )?];
            staged.push(declaration(cx, lw, &format!("th@{class}"), body)?);
        }
        staged.push(self.declaration(value, resolved, cx, lw)?);
        Ok(staged)
    }

    /// The declaration of the kind `value`, guarded on its environment's
    /// name: see the [module documentation](self#the-declarations-in-the-style-file).
    fn declaration(
        &self,
        value: &TheoremElement,
        resolved: bool,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<BuildId, LowerError> {
        let env = value.name.as_str();
        let word = self.headings.word(&value.name)?;
        let word = lw.splice(cx, &word)?;
        let (mandatory, optional) = (LatexArgShape::Mandatory, LatexArgShape::Optional);
        let mut body = match (value.counter.is_some(), resolved) {
            (true, false) => {
                let mut keys = format!("thmstyle={}", value.class_name);
                if value.class.shared_counter {
                    keys.push_str(&format!(",counter={PHFTHM_SHARED_COUNTER}"));
                }
                if !value.kind.expects_proof {
                    keys.push_str(",proofref=false");
                }
                let keys = vec![raw(cx, lw, &keys)?];
                let name = vec![raw(cx, lw, env)?];
                vec![command(cx, lw, "phfMakeTheorem", vec![optional, mandatory, mandatory], vec![keys, name, word])?]
            }
            (false, false) => {
                let mut name = vec![raw(cx, lw, "defaultproofname={")?];
                name.extend(word);
                name.push(raw(cx, lw, "}")?);
                let env = vec![raw(cx, lw, env)?];
                vec![command(cx, lw, "phfMakeProofEnv", vec![optional, mandatory], vec![name, env])?]
            }
            (true, true) => {
                let style = vec![raw(cx, lw, value.class_name.as_str())?];
                let style = command(cx, lw, "theoremstyle", vec![mandatory], vec![style])?;
                let name = vec![raw(cx, lw, env)?];
                let theorem = if value.class.shared_counter {
                    let counter = vec![raw(cx, lw, SHARED_LATEX_COUNTER)?];
                    command(cx, lw, "newtheorem", vec![mandatory, optional, mandatory], vec![name, counter, word])?
                } else {
                    command(cx, lw, "newtheorem", vec![mandatory, mandatory], vec![name, word])?
                };
                // `amsthm`'s `\newtheorem` defines globally; the group keeps
                // the `\theoremstyle` it sets from reaching what follows the
                // style file.
                vec![raw(cx, lw, "\\begingroup")?, style, theorem, raw(cx, lw, "\\endgroup")?]
            }
            (false, true) => {
                // An environment over `amsthm`'s `proof`, its default title
                // the kind's word.
                let mut body = vec![raw(cx, lw, &format!("\\newenvironment{{{env}}}[1][{{"))?];
                body.extend(word);
                body.push(raw(cx, lw, "}]{\\begin{proof}[{#1}]}{\\end{proof}}")?);
                body
            }
        };
        if value.counter.is_some() {
            body.extend(self.reference_type(value, resolved, cx, lw)?);
        }
        declaration(cx, lw, env, body)
    }

    /// The reference type of the numbered kind `value` for the LaTeX
    /// family's reference command, `\zcref` of the `zref-clever` package, as
    /// two pieces of LaTeX:
    ///
    /// - the type, named after the kind, with the kind's own words mapped to
    ///   the package's keys as every reference type of the family maps them
    ///   ([`refs::reference_type_words`]):
    ///   `\zcRefTypeSetup{lemma}{Name-sg={Lemma},name-sg={lemma},…}`, with the
    ///   abbreviated keys too (`Name-sg-ab={Lem.}`, …) where the kind declares
    ///   the variant `abbreviated`;
    /// - the counter that the kind's environment steps, mapped to that type
    ///   inside the environment
    ///   (`\AddToHook{env/lemma/begin}{\zcsetup{countertype={<counter>=lemma}}}`),
    ///   so that a reference to a lemma counting in a shared counter prints
    ///   "lemma". The counter is the one the declaration makes the environment
    ///   step: in the authored form, the alias counter that `phfthm` creates
    ///   under the kind's name for a kind in the shared counter, and the
    ///   counter `phfthm@<kind>` it creates for a kind with a counter of its
    ///   own; in the resolved form, the shared counter, or the kind's own
    ///   counter under its name.
    ///
    /// Both are written inside the kind's declaration, so a kind defined
    /// before the style file is loaded keeps the reference type its own
    /// definition gives it.
    fn reference_type(
        &self,
        value: &TheoremElement,
        resolved: bool,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Vec<BuildId>, LowerError> {
        let env = value.name.as_str();
        let counter = match (value.class.shared_counter, resolved) {
            (true, false) | (false, true) => env.to_string(),
            (false, false) => format!("phfthm@{env}"),
            (true, true) => SHARED_LATEX_COUNTER.to_string(),
        };
        let words = self.headings.reference_words(&value.name)?;
        let mut body = refs::stage_reference_type_setup(cx, lw, env, &words)?;
        body.push(raw(
            cx,
            lw,
            &format!(
                "\\AddToHook{{env/{env}/begin}}{{\\zcsetup{{countertype={{{counter}={env}}}}}}}"
            ),
        )?);
        Ok(body)
    }
}

/// The rule of the three nodes of an instantiated heading, the name
/// ([`NAME_ELEMENT`]), the title pointer ([`TITLE_POINTER`]) and the
/// back-reference ([`RELATION`]), on the `latex-recomposed` backend: the
/// node's own body, lowered through the rules, and nothing around it. The
/// body is the kind's word for a name node and empty for the other two.
///
/// The three nodes occur only inside a heading that the general theorem rule
/// instantiates from the class's heading template. This backend leaves the
/// heading to LaTeX and instantiates no template, so the nodes never reach
/// it. The rule exists because a plan checks that its backend can lower every
/// element a document may hold.
#[derive(Debug, Default, Clone, Copy)]
struct HeadingNodeLatexRule;

impl Lower for HeadingNodeLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(lw.lower_body(cx, &node)?))
    }
}

/// The rules of the three nodes of a heading on the `latex-recomposed`
/// backend ([`HeadingNodeLatexRule`]), each the default there only.
fn heading_node_latex_rules() -> [LoweringRule; 3] {
    [(NAME_LATEX_RULE, NAME_ELEMENT), (TITLE_POINTER_LATEX_RULE, TITLE_POINTER), (RELATION_LATEX_RULE, RELATION)]
        .map(|(name, element)| {
            LoweringRule::new(
                name,
                element,
                LoweringDecl::new(),
                Arc::new(HeadingNodeLatexRule),
                DefaultMark::OnBackends(vec![BACKEND.into()]),
            )
            .docs(
                "a node of a theorem's heading lowers to its own body; the recomposed LaTeX \
                 instantiates no heading, because LaTeX writes a theorem's heading, so the node \
                 never reaches this backend",
            )
        })
}

/// The theorem's rule, the default on the `latex-recomposed` backend only.
fn latex_rule(headings: Arc<Headings>) -> LoweringRule {
    LoweringRule::new(
        THEOREM_LATEX_RULE,
        THEOREM,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .reads_opt(VALUE_SAFE_LABELS)
            .option::<LatexRecomposedRenderOptions>()
            .produces(DEFINITION)
            .produces(ENVIRONMENT_CONSTRUCT)
            .produces(MACRO)
            .produces(RAW),
        Arc::new(TheoremLatexRule { headings }),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a theorem lowers to the environment of its kind's name around its title, its \
         labels in the LaTeX hook `\\flmLabels`, `\\noproofref` where written, and its body, \
         with the kind's declaration for the style file; a proof's title is written in the \
         LaTeX hook `\\flmProofTitle`, and its back-reference as written, or, under the \
         render option `resolved_proof_refs`, as a reference in `\\flmProofTitle`",
    )
}

/// Registers the theorem's rule and the rules of the three nodes of a
/// heading ([`HeadingNodeLatexRule`]), each the default on the
/// `latex-recomposed` backend only, and contributes the feature's definition
/// piece and the definition piece of each form ([`definition_piece`],
/// [`authored_piece`], [`resolved_piece`]) to the service slot of definition
/// pieces ([`preamble::SLOT`]). The names they use that other features own
/// (the backend, the LaTeX family's construct elements, the safe-labels
/// tables, the service slot of definition pieces) are declared through peer
/// declarations ([`PeerName`](crate::core::registry::PeerName)), so that a
/// registry without them still builds, with these rules set aside (excluded
/// from rule selection).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder.
pub(super) fn register(b: &mut FeatureBuilder<'_>, headings: Arc<Headings>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the feature's LaTeX rules are the default",
    )?;
    b.peer_element::<LatexDefinitionPayload>(
        DEFINITION,
        "the LaTeX family's definition construct, which the theorem's rule stages for the \
         declarations of the kinds",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        ENVIRONMENT_CONSTRUCT,
        "the LaTeX family's environment construct, which the theorem's rule stages",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the rules stage for the labels, \
         `\\noproofref`, and the LaTeX hooks `\\flmProofTitle` (a proof's title) and `\\flmRef` \
         (a reference)",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, in which the theorem's rule writes a \
         back-reference's stars, the declarations' fixed LaTeX, and a label whose label type \
         the references feature marks as raw (written byte for byte)",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the rules read for the safe label \
         written for a label",
    )?;
    b.peer_table::<ValueSafeLabels>(
        VALUE_SAFE_LABELS.name(),
        "the references feature's value names, which the rules read for the safe label of a \
         back-reference in a proof inside a synthesized value",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(authored_piece()))?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(resolved_piece()))?;
    b.lowering_rule(latex_rule(headings))?;
    for rule in heading_node_latex_rules() {
        b.lowering_rule(rule)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_is_named_as_its_identifier_says_and_provides_noproofref() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.content.contains("\\ProvideDocumentCommand\\noproofref{}{}"));
        assert!(!piece.content.contains("flmProofTitle"));
        assert!(piece.requires.is_empty());
    }

    #[test]
    fn each_form_s_piece_provides_the_proof_title_hook_with_its_own_default() {
        let authored = authored_piece();
        assert_eq!(authored.id, piece_id(AUTHORED_PIECE_NAME));
        assert_eq!(authored.id.as_str(), AUTHORED_PIECE);
        assert!(authored.content.contains("\\ProvideDocumentCommand\\flmProofTitle{m}{#1}\n"));
        assert_eq!(authored.requires, [Name::from(packages::PHFTHM)]);
        let resolved = resolved_piece();
        assert_eq!(resolved.id, piece_id(RESOLVED_PIECE_NAME));
        assert_eq!(resolved.id.as_str(), RESOLVED_PIECE);
        assert!(
            resolved.content.contains("\\ProvideDocumentCommand\\flmProofTitle{m}{\\proofname\\space (#1)}\n")
        );
        assert_eq!(resolved.requires, [Name::from(packages::AMSTHM)]);
    }
}
