//! The lowering rules of the [`xnotes`](super) feature for the
//! `latex-recomposed` backend (its recomposition rules): each xnote is written back as a LaTeX command
//! of its xnote kind's own, `\flmXN<kind>`, which the style file defines and a
//! LaTeX author may redefine.
//!
//! | written | LaTeX |
//! |---|---|
//! | `\footnote{Except the empty one.}` | `\flmXNfootnote{Except the empty one.}` |
//! | `\footnote[\label{fn:a}]{See the appendix.}` | `\flmXNfootnote[{\flmLabels{\label{ref1}}}]{See the appendix.}` |
//! | `\footnote{First paragraph.`, a blank line, `Second paragraph.}` | `\flmXNfootnote{First paragraph.`, a blank line, `Second paragraph.}` |
//! | `\endnote{…}`, a second kind `endnote` | `\flmXNendnote{…}` |
//!
//! # The command
//!
//! An xnote of the xnote kind `<kind>` is written
//! `\flmXN<kind>[<labels>]{<content>}` (the rule [`LATEX_RULE`], through
//! [`stage_latex`]). The letters `XN`, for *xnote*, identify the feature, not
//! one kind, as the letter `U` identifies the document's own definitions
//! ([`names`](crate::stdcore::backends::latex::names)): the command of every
//! kind is `\flmXN` followed by the kind's name, which is made of ASCII
//! letters ([`XnoteKind::validate`]) and so makes a LaTeX command name as
//! written.
//!
//! The optional argument is written only where the xnote carries labels. It
//! holds the labels, each as its safe label (the key the recomposed LaTeX
//! writes for an FLM label, `ref1`, `ref2`, …; see
//! [`SafeLabels`]), wrapped in the
//! LaTeX hook `\flmLabels` exactly as a heading's labels are
//! ([`stage_label_group`](crate::stdcore::features::refs::stage_label_group);
//! with the render option
//! [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! off, the labels are written bare). A LaTeX hook is a macro that the
//! recomposed LaTeX writes and the style file defines with a plain default,
//! which a user redefines in the document's preamble (see
//! [`preamble`] and [the LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks)). The content is written
//! through the ordinary rules: text escaped, commands as their rules write
//! them, a blank line as a blank line, so that a note of two paragraphs is two
//! paragraphs inside the argument.
//!
//! The same command is written regardless of the kind's placement: a kind
//! that the other outputs show in place is passed to LaTeX like any other,
//! and a LaTeX author who wants it set differently redefines the command.
//! The refused content of the optional argument
//! ([`BadLabelArgument`](super::BadLabelArgument)) and an empty xnote
//! ([`Empty`](super::Empty)) are reported as on every output.
//!
//! # Nested xnotes and xnotes in synthesized values
//!
//! An xnote written inside another is written as any xnote, labels included,
//! as a command inside the other's argument, and LaTeX sets it as it sets a
//! footnote inside a footnote: LaTeX's own `\footnote` prints the inner
//! note's mark and number and leaves the inner note's text unset, so a label
//! on the inner note is undefined there. No short definition of the command
//! from LaTeX's own footnote commands (`\footnotemark`, `\footnotetext`) sets
//! nested notes, so the default definition leaves them to the LaTeX author,
//! who redefines the command to set them.
//!
//! An xnote in content lowered as a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) (the template
//! of a user definition kept as a call, which the style file holds, or a
//! list's own label template) is written as its command there too: LaTeX
//! sets the note wherever the value is used, and no missing record is
//! reported. The other outputs write no mark for an xnote in content that the
//! run builds itself, apart from the document's trees (such as a list's own
//! label template), and report that the table of xnotes holds no record for
//! it.
//!
//! # Numbers in LaTeX and in FLM
//!
//! Nothing in this module reads the table of xnotes or a number: LaTeX
//! numbers the notes it sets. For a kind numbered through the whole document
//! and passed to LaTeX's `\footnote` (the default kind `footnote`), the
//! numbers LaTeX prints are the ones FLM computes, because both count in
//! document order; a nested xnote keeps them in step too, since FLM numbers
//! it after the xnote that holds it and LaTeX numbers it in the same order.
//! The numbers differ in these cases:
//!
//! - A kind numbered within another counter
//!   ([`within`](super::XnoteKind::within)) restarts its numbers in FLM where
//!   that counter advances; LaTeX restarts the counter of the kind's LaTeX
//!   command only where the document class or the author's preamble resets
//!   it.
//! - A second xnote kind that is also passed to `\footnote` (the default
//!   [`latex_command`](super::XnoteKind::latex_command)) shares LaTeX's
//!   footnote counter, so its numbers in LaTeX continue from the footnotes'.
//!   A LaTeX command with a counter of its own keeps them apart.
//! - An xnote in a list's own label template is set by LaTeX beside every
//!   item's label, and again inside every `\ref` to an item that prints the
//!   label template (every `\ref` to an item of a list with no reference
//!   template of its own, since the `enumitem` package's `ref=` defaults to
//!   `label=`), whereas the other outputs write no mark there. Every later
//!   note of that counter is then numbered higher in LaTeX, by as many notes.
//!
//! # The definition in the style file
//!
//! Every xnote kind has a **definition piece** of its own,
//! `flm.latex.def.xnotes.<kind>` ([`definition_piece`]): a unit of LaTeX
//! definitions that a feature registers under an identifier
//! ([`LatexPreambleDef`]). The rule requires it, so the style file holds it
//! exactly when the document holds an xnote of that kind. For the kind
//! `footnote` it reads
//!
//! ```latex
//! % \flmXNfootnote[<labels>]{<note>}: a note of the kind `footnote' in the FLM document;
//! % by default it passes the note, its labels first, to \footnote.
//! % Redefine it to set these notes another way.
//! \ProvideDocumentCommand\flmXNfootnote{+O{} +m}{\footnote{#1#2}}
//! ```
//!
//! The default definition passes the note to the kind's
//! [`latex_command`](XnoteKind::latex_command), with its labels first inside
//! that command's argument: LaTeX's `\footnote` makes a label written in its
//! text refer to the footnote's number, so a reference to the label prints
//! the footnote's number. The command is defined with
//! `\ProvideDocumentCommand`, the guarded form of a macro that a definition
//! piece offers a user ([how a piece is
//! written](crate::stdcore::backends::latex::preamble#how-a-piece-is-written)),
//! so a definition the author made before loading the style file takes
//! precedence. The command is robust, so an xnote inside a heading's title
//! survives being written to the table of contents, and `+` lets its argument
//! hold a paragraph break.
//!
//! The piece requests no LaTeX package: the author decides which LaTeX
//! command sets a kind, and a kind whose command needs a package (an
//! `endnote` kind passed to `\endnote`, which the `endnotes` package defines)
//! gets the package from the author's own preamble, through the render
//! options that add packages and preamble text
//! ([`extra_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::extra_packages),
//! [`extra_preamble`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::extra_preamble)).
//! An unnumbered kind gets the same default; a LaTeX author who wants such
//! notes set as asides redefines the command, for instance
//! `\ProvideDocumentCommand\flmXNsidenote{+O{} +m}{~[#1#2]}`, before loading
//! the style file.
//!
//! # The list, the content pointer and content shown again
//!
//! The recomposed LaTeX places no list of xnotes by default: LaTeX sets each
//! xnote where its command is written, and the backend places only the
//! document-level pieces ([`Piece`](crate::core::piece::Piece)) that a render
//! request lists for it
//! ([`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions)).
//! The list element of every kind ([`list_element_name`]) still has a rule on
//! this backend, [`ListLatexRule`], which writes nothing, so that a list a
//! render places anyway writes nothing, and because a plan checks that its
//! backend can lower every element the registry holds; so do the content
//! pointer that the lists hold ([`PointerLatexRule`]) and the node that
//! represents an xnote in content shown again ([`MarkOfLatexRule`]).
//!
//! # Registration
//!
//! The rules are marked default on the `latex-recomposed` backend only. They
//! are registered under peer declarations
//! ([`PeerName`](crate::core::registry::PeerName)) of that backend, of the
//! LaTeX constructs they stage and of the table they read. The `xnotes`
//! feature registers its own rules (the xnote's, the content pointer's, the
//! mark-of node's) with these peer declarations, and
//! [`contribute_kind`](super::contribute_kind) registers the list rule of
//! each xnote kind with them, on behalf of the feature that contributes the
//! kind. A registry without the LaTeX features therefore
//! builds, with the rules set aside (excluded from rule selection), and a
//! feature whose own rule calls [`stage_latex`] declares nothing more.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name, RuleName};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::preamble::{self, LatexPreambleDef};
use crate::stdcore::backends::latex::{LatexArgShape, LatexMacroPayload, LatexRawPayload};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::features::refs::{self, LabelUse, SAFE_LABELS, SafeLabels};

use super::config::XnoteKind;
use super::lower::{report_empty, report_labels, Content};
use super::{labels_of, list_element_name, XnoteElement, CONTENT_POINTER, MARK_OF, XNOTE};

/// The name of the recomposition rule of an xnote (its lowering rule on the
/// `latex-recomposed` backend), `flm.xnotes.xnote.lower.latex-recomposed`
/// ([`XnoteLatexRule`]).
pub const LATEX_RULE: &str = "flm.xnotes.xnote.lower.latex-recomposed";

/// The prefix of the LaTeX command that writes an xnote, `flmXN`: the command
/// of the xnote kind `<kind>` is `\flmXN<kind>` ([`command_name`]).
pub const COMMAND_PREFIX: &str = "flmXN";

/// The LaTeX command, without the backslash, that writes an xnote of the
/// xnote kind `kind` and that its definition piece defines: `flmXN<kind>`
/// (`flmXNfootnote`).
pub fn command_name(kind: &str) -> String {
    format!("{COMMAND_PREFIX}{kind}")
}

/// The identifier of the definition piece of the xnote kind `kind`,
/// `flm.latex.def.xnotes.<kind>` ([`definition_piece`]).
pub fn definition_piece_name(kind: &str) -> Name {
    preamble::piece_id(&format!("xnotes.{kind}"))
}

/// The name of the lowering rule of the list element of the xnote kind
/// `kind` on the `latex-recomposed` backend,
/// `flm.xnotes.<kind>.list.lower.latex-recomposed` ([`ListLatexRule`]).
pub fn list_rule_name(kind: &str) -> RuleName {
    RuleName::from(format!("flm.xnotes.{kind}.list.lower.latex-recomposed"))
}

/// The definition piece of the xnote kind `kind`,
/// `flm.latex.def.xnotes.<kind>` ([`LatexPreambleDef`]): the style file's
/// definition of `\flmXN<kind>`, which passes the note, its labels first, to
/// the kind's [`latex_command`](XnoteKind::latex_command).
///
/// The piece defines one macro, `\flmXN<kind>[<labels>]{<note>}`, with
/// `\ProvideDocumentCommand`: guarded, so that the author's own definition
/// made before the style file is loaded takes precedence; robust, so that it
/// survives in a heading's title; and with long arguments (`+`), so that a
/// note may hold a paragraph break. Its default is
/// `\<latex_command>{#1#2}`. It requires nothing: the LaTeX command to which a
/// kind is passed is the author's, loaded through the author's own preamble
/// where a package defines it. Redefine `\flmXN<kind>` to set a kind's notes
/// differently. See the [module documentation](self).
pub fn definition_piece(kind: &XnoteKind) -> LatexPreambleDef {
    let name = kind.name.as_str();
    let command = command_name(name);
    let target = &kind.latex_command;
    LatexPreambleDef::new(
        &format!("xnotes.{name}"),
        format!(
            "% \\{command}[<labels>]{{<note>}}: a note of the kind `{name}' in the FLM document;\n\
             % by default it passes the note, its labels first, to \\{target}.\n\
             % Redefine it to set these notes another way.\n\
             \\ProvideDocumentCommand\\{command}{{+O{{}} +m}}{{\\{target}{{#1#2}}}}\n"
        ),
        format!(
            "`\\{command}[<labels>]{{<note>}}`, the command that writes an xnote of the xnote \
             kind `{name}`; by default it passes the note, its labels first, to \
             `\\{target}`"
        ),
    )
}

/// Stages (builds into the lowered tree, through `lw`; see
/// [`Lowerer`]) one xnote that the node `node` carries, of the
/// xnote kind `kind`, as the recomposed LaTeX writes it: the command
/// `\flmXN<kind>` ([`command_name`]), with an optional argument holding
/// `labels` (each as its safe label, in the LaTeX hook `\flmLabels` unless
/// the render option `label_hooks` is off) where there is a label,
/// and a mandatory argument holding the xnote's content lowered through the
/// ordinary rules. It requires (records as a requirement of the output) the
/// kind's definition piece ([`definition_piece_name`]).
///
/// It is the only function that writes an xnote on this backend: the rule of
/// an author's `\footnote` calls it, and so does the recomposition rule of an
/// element of another feature that carries xnotes of a kind that feature
/// contributed ([`contribute_kind`](super::contribute_kind)).
///
/// **What the caller passes.** `nth` is the xnote's position among the
/// node's xnotes of the kind, counting from 0, and its content is the one
/// that the node's [`XnoteSource`](super::XnoteSource) aspect returns at that
/// position, under the pairing that aspect states: an element carrying one
/// xnote of the kind passes `0`, and an element carrying several calls the
/// function once for each, each xnote written as a command of its own.
/// `labels` are the labels that the caller's element read for the xnote, the
/// ones it passed to the [`ref_points`](super::ref_points) call of that
/// xnote.
///
/// **What the calling rule must declare.** It must declare that it produces
/// `flm.pres.latex.macro` and `flm.pres.latex.raw` (a raw node: LaTeX text
/// written as it is), that it may read the references feature's safe-labels
/// table (`flm.labels.safe`, an optional read: without it, a label is written
/// as the document wrote it), and that it reads the render options
/// [`LatexRecomposedRenderOptions`].
///
/// **Other cases.**
///
/// - A position `nth` beyond the node's xnotes of the kind is written as the
///   command with no content, and reported as the Warning
///   [`Empty`](super::Empty).
/// - An xnote inside another xnote's content is written as any other, labels
///   included, and LaTeX sets a note inside a note as it does (under the
///   default definition, its own `\footnote` prints the inner mark and leaves
///   the inner text unset).
/// - An xnote in content lowered as a synthesized value (the template of a
///   user definition kept as a call, which is written into the style file; a
///   list's own label template; an instantiated use site) is written as the
///   command, as anywhere else, so that LaTeX sets the note wherever the
///   value is used. The recomposed LaTeX never leaves an xnote out, whereas
///   the other outputs write no mark for an xnote in content that the run
///   builds itself, apart from the document's trees. Its labels are written as anywhere else; in practice a
///   synthesized value holds none, since a value that declares a label is
///   refused before it is lowered.
///
/// An xnote with no content is still written, and is reported as the Warning
/// [`Empty`](super::Empty). The function does not report what an element's
/// own argument holds besides its labels: the caller does, as the rule of
/// `\footnote` reports [`BadLabelArgument`](super::BadLabelArgument).
///
/// # Errors
///
/// [`LowerError`] when `node` is not a node of the tree being lowered, when
/// the calling rule did not declare what it reads or produces, and from the
/// lowering of the content and the staging of the nodes.
pub fn stage_latex(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
    labels: &[LabelUse],
) -> Result<Vec<BuildId>, LowerError> {
    // Written wherever it is lowered, a value included: the template of a
    // definition kept as a call is lowered as one into the style file, and
    // the note must reach the page wherever the call stands.
    report_empty(lw, node, kind, nth)?;
    let mut shapes = Vec::new();
    let mut slots = Vec::new();
    if !labels.is_empty() {
        let mut staged = Vec::new();
        for label in labels {
            staged.push(refs::stage_label(cx, lw, &label.text, label.label.as_ref())?);
        }
        shapes.push(LatexArgShape::Optional);
        slots.push((ARG_SLOT.to_string(), refs::stage_label_group(cx, lw, staged, false)?));
    }
    let content = lower_run(cx, lw, Content::of(node, kind, nth).nodes())?;
    shapes.push(LatexArgShape::Mandatory);
    slots.push((ARG_SLOT.to_string(), content));
    lw.require(&definition_piece_name(kind.name.as_str()));
    let command = lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(command_name(kind.name.as_str()), shapes),
        slots,
    )?;
    Ok(vec![command])
}

/// The recomposition rule of an xnote, [`LATEX_RULE`]: it reports what the
/// optional argument holds besides `\label`, as every rule of the element
/// does, then writes the xnote through [`stage_latex`], with the labels of its
/// optional argument ([`labels_of`]). The rule does not read the kind's
/// placement: see the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct XnoteLatexRule;

impl Lower for XnoteLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<XnoteElement>(&node)
            .ok_or_else(|| LowerError::structure("an xnote without its value"))?;
        let kind = Arc::clone(&value.kind);
        report_labels(lw, &node, &kind);
        let (labels, _) = labels_of(&node);
        // The element carries one xnote, the first of its kind.
        Ok(Lowering::Emit(stage_latex(cx, lw, &node, &kind, 0, &labels)?))
    }
}

/// The lowering rule of an xnote kind's list element on the
/// `latex-recomposed` backend ([`list_rule_name`]): it writes nothing. LaTeX
/// sets each xnote where its command is written, so a list of xnotes, placed
/// only where a render request lists it for this backend, writes nothing; the
/// rule also exists because a plan checks that its backend can lower every
/// element the registry holds.
#[derive(Debug, Default, Clone, Copy)]
pub struct ListLatexRule;

impl Lower for ListLatexRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// The name of the rule of the content pointer on the `latex-recomposed`
/// backend, `flm.xnotes.content-pointer.lower.latex-recomposed`
/// ([`PointerLatexRule`]).
pub const POINTER_LATEX_RULE: &str = "flm.xnotes.content-pointer.lower.latex-recomposed";

/// The lowering rule of the content pointer ([`CONTENT_POINTER`]) on the
/// `latex-recomposed` backend, [`POINTER_LATEX_RULE`]: it writes nothing. A
/// pointer is placed only in a list of xnotes, which this backend's list
/// rule writes as nothing; the rule exists because a plan checks that its backend can lower
/// every element the registry holds.
#[derive(Debug, Default, Clone, Copy)]
pub struct PointerLatexRule;

impl Lower for PointerLatexRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// The lowering rule of the content pointer on the `latex-recomposed` backend
/// ([`PointerLatexRule`], [`POINTER_LATEX_RULE`]), marked default on that
/// backend only.
pub fn pointer_latex_rule() -> LoweringRule {
    LoweringRule::new(
        POINTER_LATEX_RULE,
        CONTENT_POINTER,
        LoweringDecl::new(),
        Arc::new(PointerLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a content pointer lowers to nothing: the recomposed LaTeX places no list of xnotes, \
         because LaTeX sets each xnote where its command is written",
    )
}

/// The name of the lowering rule of the node that represents an xnote in
/// content shown again ([`MARK_OF`]) on the `latex-recomposed` backend,
/// `flm.xnotes.mark-of.lower.latex-recomposed` ([`MarkOfLatexRule`]).
pub const MARK_OF_LATEX_RULE: &str = "flm.xnotes.mark-of.lower.latex-recomposed";

/// The lowering rule of the node that represents an xnote in content shown
/// again ([`MARK_OF`]) on the `latex-recomposed` backend,
/// [`MARK_OF_LATEX_RULE`]: it writes nothing and reports nothing.
///
/// No rule of the standard package lowers content shown again on this
/// backend, so no such node reaches it: a reference is written back as `\ref`
/// for LaTeX to resolve; the title that a reference to a heading shows is the
/// one LaTeX reuses through `\flmHeading`; and the display text of a float or
/// a defined term is written as the element's own words, with an xnote there
/// written as its command. Printing the mark of the xnote the node represents
/// would need a key by which LaTeX knows that xnote, and the backend assigns
/// no such key to any xnote. The rule exists because a plan checks that its
/// backend can lower every element the registry holds.
#[derive(Debug, Default, Clone, Copy)]
pub struct MarkOfLatexRule;

impl Lower for MarkOfLatexRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// The lowering rule of the node that represents an xnote in content shown
/// again on the `latex-recomposed` backend ([`MarkOfLatexRule`],
/// [`MARK_OF_LATEX_RULE`]), marked default on that backend only.
pub fn mark_of_latex_rule() -> LoweringRule {
    LoweringRule::new(
        MARK_OF_LATEX_RULE,
        MARK_OF,
        LoweringDecl::new(),
        Arc::new(MarkOfLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "the mark of an xnote in content shown again lowers to nothing: no rule of the standard \
         package lowers content shown again on the recomposed LaTeX",
    )
}

/// The recomposition rule of an xnote, [`LATEX_RULE`] ([`XnoteLatexRule`]),
/// marked default on the `latex-recomposed` backend only.
pub fn latex_rule() -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        XNOTE,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .option::<LatexRecomposedRenderOptions>()
            .produces(MACRO)
            .produces(RAW),
        Arc::new(XnoteLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "an xnote lowers to `\\flmXN<kind>`, with its labels as the optional argument where it \
         has any (in the LaTeX hook `\\flmLabels` unless the render option `label_hooks` is \
         off), and its content, lowered through the rules, as the mandatory argument",
    )
}

/// The lowering rule of the list element of the xnote kind `kind` on the
/// `latex-recomposed` backend ([`ListLatexRule`], [`list_rule_name`]), marked
/// default on that backend only.
pub fn list_latex_rule(kind: &str) -> LoweringRule {
    LoweringRule::new(
        list_rule_name(kind),
        list_element_name(kind),
        LoweringDecl::new(),
        Arc::new(ListLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "the list of a kind's xnotes lowers to nothing, because LaTeX sets each xnote where its \
         command is written",
    )
}

/// Registers the `xnotes` feature's own recomposition rules: the content
/// pointer's ([`pointer_latex_rule`]), the one of the node that represents an
/// xnote in content shown again ([`mark_of_latex_rule`]), and, where the
/// configuration declares an xnote kind with a command (where the element
/// [`XNOTE`] exists), the xnote's ([`latex_rule`]); with the peer
/// declarations they need ([`declare_peers`]).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// in a [registration round](crate::core::registry::builder#the-two-registration-rounds)
/// that does not accept it (from a finalizer, for instance).
pub(super) fn register(b: &mut FeatureBuilder<'_>, with_element: bool) -> Result<(), BuildError> {
    declare_peers(b)?;
    b.lowering_rule(pointer_latex_rule())?;
    b.lowering_rule(mark_of_latex_rule())?;
    if with_element {
        b.lowering_rule(latex_rule())?;
    }
    Ok(())
}

/// Declares what the xnote kind `kind` needs on the LaTeX side, on behalf of
/// the feature that `b` registers: its definition piece
/// ([`definition_piece`]), as a peer contribution to the service slot of
/// definition pieces ([`preamble::SLOT`]); the rule of its list element on
/// the `latex-recomposed` backend ([`list_latex_rule`]); and the peer
/// declarations of the names that this rule and a rule calling
/// [`stage_latex`] use ([`declare_peers`]).
/// [`contribute_kind`](super::contribute_kind) calls it once per kind.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// in a [registration round](crate::core::registry::builder#the-two-registration-rounds)
/// that does not accept it (from a finalizer, for instance).
pub(super) fn declare_kind(b: &mut FeatureBuilder<'_>, kind: &XnoteKind) -> Result<(), BuildError> {
    declare_peers(b)?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece(kind)))?;
    b.lowering_rule(list_latex_rule(kind.name.as_str()))
}

/// Makes, on behalf of the feature that `b` registers, a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of every name that the
/// LaTeX rules of this module and a rule calling [`stage_latex`] use and that
/// the LaTeX features or the references feature own: the `latex-recomposed`
/// backend, the macro and raw constructs and the safe-labels table. A
/// feature therefore builds in a registry without the LaTeX features, with
/// its LaTeX rules set aside (excluded from rule selection). Every call
/// declares every name: the registry merges an identical repeated peer
/// declaration into the first.
fn declare_peers(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    // Declared for the calling feature on every call, per kind and for the
    // feature's own rule alike.
    // See decision rationale at [§flm-dd-dr:std-xnote-latex-peers].
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rules are the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages for \
         `\\flmXN<kind>` and for the labels",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, in which a label is written as the document \
         wrote it",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the recomposition rule reads for the \
         safe label of each label of an xnote",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::names::USER_DEFINITION_PREFIX;
    use crate::stdcore::features::numbering::PrefixWords;

    #[test]
    fn the_footnote_piece_is_the_designed_definition() {
        let piece = definition_piece(&XnoteKind::footnote());
        assert_eq!(piece.id.as_str(), "flm.latex.def.xnotes.footnote");
        assert!(piece.requires.is_empty());
        assert_eq!(
            piece.content,
            "% \\flmXNfootnote[<labels>]{<note>}: a note of the kind `footnote' in the FLM \
             document;\n\
             % by default it passes the note, its labels first, to \\footnote.\n\
             % Redefine it to set these notes another way.\n\
             \\ProvideDocumentCommand\\flmXNfootnote{+O{} +m}{\\footnote{#1#2}}\n"
        );
    }

    #[test]
    fn a_kind_s_piece_names_its_own_command_and_hands_the_note_to_its_latex_command() {
        let kind = XnoteKind::numbered("endnote", PrefixWords::new("Endnote~", "Endnotes~"))
            .latex_command("endnote");
        let piece = definition_piece(&kind);
        assert_eq!(piece.id, definition_piece_name("endnote"));
        assert!(piece.content.contains("\\ProvideDocumentCommand\\flmXNendnote{+O{} +m}{\\endnote{#1#2}}\n"));
        // The piece's names are the family's, never the document's.
        assert!(!piece.content.contains(USER_DEFINITION_PREFIX));
    }

    #[test]
    fn the_rules_are_the_defaults_on_the_recomposed_backend_only() {
        let rule = latex_rule();
        assert_eq!(rule.name, LATEX_RULE);
        assert_eq!(rule.element, XNOTE);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
        assert!(rule.decl.reads.is_empty());
        let list = list_latex_rule("footnote");
        assert_eq!(list.name.as_str(), "flm.xnotes.footnote.list.lower.latex-recomposed");
        assert_eq!(list.element.as_str(), "flm.xnotes.footnote.list");
        assert!(list.default.is_specific_to(&BACKEND.into()));
        assert!(list.decl.produces.is_empty());
    }
}
