//! The lowering rules of the [`citations`](super) feature for the
//! `latex-recomposed` backend (the recomposed LaTeX): a citation is written
//! back as the author wrote it, through three LaTeX hooks, and the list of
//! cited works is left to LaTeX.
//!
//! A LaTeX hook is a macro that the recomposed LaTeX writes and the style
//! file defines with a plain default, which a user redefines in the
//! document's preamble to change what the recomposed LaTeX does at that point
//! (see [LaTeX hooks](crate::stdcore::backends::latex::preamble) and the
//! rendering guide's [LaTeX hooks](crate::guide::rendering#the-latex-hooks)).
//! The three hooks of a citation are `\flmCite`, `\flmCitePreSpace` and
//! `\flmRefCite`:
//!
//! | written | LaTeX |
//! |---|---|
//! | `text~\cite{a,b,c}` | `text~\flmCitePreSpace\flmCite{a,b,c}` |
//! | `\cite[Thm 4]{a,b}\cite{c}` | `\flmCitePreSpace\flmCite[Thm 4]{a,b}\flmCite{c}` |
//! | `\cite{arxiv:1234_5}` | `\flmCitePreSpace\flmCite{arxiv:1234_5}` |
//! | `\R\cite{a,b,c}` | `\flmRefCite{refs.~}{\flmCite{a,b,c}}` |
//! | `\R[S]\cite{a}\cite{b}` | `\flmRefCite{Refs.~}{\flmCite{a}\flmCite{b}}` |
//! | `\R[I]\cite{a}` | `\flmRefCite{}{\flmCite{a}}` |
//!
//! The rendering guide shows what LaTeX prints for them ([Citations in the
//! recomposed LaTeX](crate::guide::rendering#citations-in-the-recomposed-latex)).
//!
//! # A citation written as the author wrote it
//!
//! This backend lowers the tree before its block structure is read, so the
//! author's whitespace is written to the output unchanged. The rule of
//! `\cite` ([`CiteLatexRule`]) writes **one** `\flmCitePreSpace` for the
//! citation chunk, then **one** `\flmCite` per call of the citation chunk:
//! the first call and each further call, in order, each with its own citation
//! keys and its own note. Nothing is sorted, merged or compressed: LaTeX
//! numbers the cited works, and sorts and compresses them (or not) as the
//! author's bibliography setup says. The rule of `\R` ([`RefCiteLatexRule`])
//! writes no `\flmCitePreSpace`, because an `\R` is a part of its sentence,
//! and passes its word and its calls to `\flmRefCite`. The word is the
//! citation counter style's word: singular when the citation chunk holds one
//! item and plural otherwise, counting a citation key written twice as two
//! items (the other backends count the distinct numbers the citation mark
//! shows; see [`StagedMark::shown`](super::StagedMark::shown)), capitalized
//! under the flag `S`, and empty under the flag `I`. The configuration's placement and
//! form change nothing here: LaTeX decides how a citation is shown.
//!
//! The **citation keys** of one call are written as a raw node (the LaTeX
//! family's construct `flm.pres.latex.raw`, written to the output without
//! escaping), joined by commas: each citation key as
//! [`read_chunk`](super::read_chunk) reads it, the FLM code written for it
//! with every run of whitespace made one space and the ends trimmed. No
//! character is escaped (an underscore in a citation key stays an
//! underscore) and no safe label is assigned, because a citation key is not
//! a label of the document but the name of an entry of the author's
//! bibliography database. A citation key that LaTeX cannot read is the
//! author's to fix.
//!
//! The citation keys are checked for placeholders
//! ([`spelling_at_lowering`](crate::core::lowering::spelling_at_lowering)), so
//! a user definition whose template holds a placeholder among the citation
//! keys of a `\cite` (`\cite{arxiv:#1}`) is not written as a LaTeX
//! definition: each of its use sites is expanded, and the rule writes the
//! citation keys that the use site supplies. A `\cite` with no placeholder in
//! its citation keys, inside a user definition kept as a LaTeX definition, is
//! written into that definition, and LaTeX numbers it wherever the
//! definition is used.
//!
//! The **note** is content, written as all content is, in the optional
//! argument; a call whose note is empty writes none.
//!
//! A call with no citation key is reported, as on every output
//! ([`report_mistakes`]), and writes no `\flmCite`.
//! A citation chunk that writes no `\flmCite` at all, and an `\R` with no
//! `\cite` directly after it ([`NoCitation`]), are
//! written as a dropped mark (`flm.pres.dropped`), which this backend writes
//! as a comment line.
//!
//! # The definition piece of the hooks
//!
//! The feature's definition piece ([`definition_piece`],
//! `flm.latex.def.citations`; a definition piece is a unit of LaTeX
//! definitions that the style file holds when the document requires it, see
//! [`LatexPreambleDef`]) defines the three LaTeX hooks, each with
//! `\ProvideDocumentCommand`, so a definition that the author's class or
//! preamble made first is the one used. It requests no LaTeX package:
//! loading `natbib` or `biblatex` is the author's choice.
//!
//! - `\flmCite[<note>]{<keys>}`, one call: LaTeX's `\cite`, with its
//!   optional argument when there is a note. An author who wants the
//!   citation entries shown in place redefines it, with `biblatex`'s
//!   `\fullcite` for example.
//! - `\flmCitePreSpace`, before a citation chunk of `\cite` calls:
//!   `\unskip~`, which removes the author's space and writes a space that
//!   does not break. An author of superscript citations redefines it to
//!   `\unskip`.
//! - `\flmRefCite{<word>}{<citations>}`, for an `\R`: the word, then the
//!   citations. An author of superscript citations who redefines `\flmCite`
//!   with a command that prints bare numbers (`natbib`'s `\citenum`) gets
//!   `refs.~1,2` with this hook unchanged.
//!
//! **A citation in a heading whose case LaTeX changes** (a class that sets
//! its running heads in capitals with `\MakeUppercase`): the definition piece
//! adds `\flmCite` to the LaTeX kernel's list of commands whose argument a
//! change of case leaves unchanged (`\l_text_case_exclude_arg_tl`, where the
//! kernel lists `\cite`), unless `\flmCite` is on it already. The kernel
//! protects the first brace group after the command. Where the note holds no
//! brace group, that group is the mandatory argument, so the citation keys
//! still refer to the author's entries, and the note is uppercased like the
//! text around it (`RESULTS [1, THM 4]`). As with LaTeX's own `\cite`, a note
//! that holds a brace group (a command's argument, as in `\emph{p.}`) takes
//! that protection instead, and the citation keys are then uppercased too.
//!
//! The definition piece also makes a citation contribute nothing to a PDF
//! bookmark where `hyperref` is loaded, before the style file or after it
//! (the word of `\flmRefCite` included), so a heading's title that holds a
//! citation gives its bookmark the title's own words.
//!
//! # The list of cited works
//!
//! No bibliography is written: the list of cited works and its entries lower
//! to nothing on this backend ([`ListNotWritten`]), and no citation entry is
//! written anywhere. LaTeX writes its own list (`thebibliography`, BibTeX,
//! `biblatex`) from the author's database. The check that every cited key has
//! a citation entry still runs, because its result belongs to the document,
//! regardless of the backend that renders it.
//!
//! # Registration
//!
//! The feature declares the `latex-recomposed` backend and the LaTeX family's
//! macro and raw constructs through peer declarations
//! ([`PeerName`](crate::core::registry::PeerName)), and contributes its
//! definition piece to the style file's service slot
//! ([`preamble::SLOT`]) as a peer contribution. A registry without the LaTeX
//! features therefore still builds, with these rules set aside (excluded from
//! rule selection; see [`PeerName`](crate::core::registry::PeerName)).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::preamble::{self, LatexPreambleDef};
use crate::stdcore::backends::latex::{LatexArgShape, LatexMacroPayload, LatexRawPayload};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::backends::pres;

use super::bibliography::{BIBLIOGRAPHY, BIBLIOGRAPHY_ENTRY};
use super::config::CAPITAL_VARIANT;
use super::diagnostics::NoCitation;
use super::elements::{calls, leading_call, read_call, Chunk, Reading, CITE, NOTE_ARG, REFCITE};
use super::mark::{read_flags, report_mistakes, stage_dropped, MarkSettings};

/// The name of the lowering rule of `\cite` on the recomposed LaTeX,
/// `flm.citations.cite.lower.latex-recomposed`, whose body is
/// [`CiteLatexRule`].
pub const CITE_LATEX_RULE: &str = "flm.citations.cite.lower.latex-recomposed";

/// The name of the lowering rule of `\R` on the recomposed LaTeX,
/// `flm.citations.refcite.lower.latex-recomposed`, whose body is
/// [`RefCiteLatexRule`].
pub const REFCITE_LATEX_RULE: &str = "flm.citations.refcite.lower.latex-recomposed";

/// The name of the feature's definition piece, `citations`, from which its
/// identifier `flm.latex.def.citations` ([`PIECE`]) is derived.
pub const PIECE_NAME: &str = "citations";

/// The identifier of the feature's definition piece,
/// `flm.latex.def.citations` ([`definition_piece`]), which the rules require
/// when they write a hook.
pub const PIECE: &str = "flm.latex.def.citations";

/// The name of the LaTeX hook that each call of a citation chunk is written
/// as, `flmCite`: `\flmCite[<note>]{<keys>}`.
pub const CITE_COMMAND: &str = "flmCite";

/// The name of the LaTeX hook written once before a citation chunk of
/// `\cite` calls, `flmCitePreSpace`, which takes no argument.
pub const PRE_SPACE_COMMAND: &str = "flmCitePreSpace";

/// The name of the LaTeX hook that an `\R` is written as, `flmRefCite`:
/// `\flmRefCite{<word>}{<citations>}`.
pub const REF_CITE_COMMAND: &str = "flmRefCite";

/// Returns the feature's definition piece, `flm.latex.def.citations`
/// ([`LatexPreambleDef`]). It defines the three LaTeX hooks, each with
/// `\ProvideDocumentCommand`: `\flmCite[<note>]{<keys>}` as LaTeX's `\cite`,
/// `\flmCitePreSpace` as `\unskip~`, and `\flmRefCite{<word>}{<citations>}`
/// as the word followed by the citations. It adds `\flmCite` to the LaTeX
/// kernel's list of commands whose argument a change of case leaves
/// unchanged, and makes a citation contribute nothing to a PDF bookmark. It
/// requests no LaTeX package.
///
/// An author redefines a hook, before loading the style file or after it, to
/// cite another way. See the [module
/// documentation](self#the-definition-piece-of-the-hooks).
pub fn definition_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% Citations, written as the document wrote them. The list of the cited works
% is LaTeX's own (thebibliography, BibTeX, biblatex), built from your
% database.
% \\flmCite[<note>]{<keys>}: one citation, its keys as written, separated by
%   commas, and its note where it has one; by default LaTeX's \\cite. To show
%   the entries in place, redefine it, with biblatex's \\fullcite for instance.
% \\flmCitePreSpace: written before citations attached to the text before
%   them; by default \\unskip~, a space that does not break. For superscript
%   citations, redefine it to \\unskip.
% \\flmRefCite{<word>}{<citations>}: citations used as a noun of the sentence,
%   with the word written before them (refs.~, Refs.~, or nothing); by
%   default the word, then the citations.
% A change of case (\\MakeUppercase) leaves the keys of \\flmCite unchanged and
% changes its note, as it does for \\cite; as for \\cite, it protects the
% first brace group after the command, so where the note holds one
% ([{...}], \\emph{...}) the keys are changed too.
\\ProvideDocumentCommand\\flmCite{o m}{\\IfValueTF{#1}{\\cite[{#1}]{#2}}{\\cite{#2}}}
\\ProvideDocumentCommand\\flmCitePreSpace{}{\\unskip~}
\\ProvideDocumentCommand\\flmRefCite{m m}{#1#2}
% What a citation contributes to a PDF bookmark: nothing. The \\AtBeginDocument
% block below runs when the document begins, so that it also applies where
% hyperref is loaded after this file; it does nothing without hyperref.
% \\flm@cite@bookmark, defined first, reads \\flmCite's optional note, if any,
% then its keys, and writes nothing.
\\def\\flm@cite@bookmark#1{\\ifx[#1\\expandafter\\flm@cite@bookmark@note\\fi}
\\def\\flm@cite@bookmark@note#1]#2{}
\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands
  \\pdfstringdefDisableCommands{%
    \\def\\flmCitePreSpace{}%
    \\let\\flmCite\\flm@cite@bookmark
    \\def\\flmRefCite#1#2{}%
  }\\fi}
\\ExplSyntaxOn
\\tl_if_in:NnF \\l_text_case_exclude_arg_tl { \\flmCite } { \\tl_put_right:Nn \\l_text_case_exclude_arg_tl { \\flmCite } }
\\ExplSyntaxOff
",
        "the LaTeX hooks of a citation: `\\flmCite[note]{keys}` (LaTeX's `\\cite` by default), \
         `\\flmCitePreSpace` and `\\flmRefCite{word}{citations}`; a change of case leaves the \
         keys of `\\flmCite` unchanged",
    )
}

/// Stages the `\flmCite` calls of the citation chunk that `node` carries, one
/// per call with at least one citation key, in order: the call's citation
/// keys as a raw node, joined by commas, and its note, where it has content,
/// as the optional argument. Returns them with the citation chunk as read.
/// Every call's writing mistakes are reported, and the definition piece
/// [`PIECE`] is required when at least one `\flmCite` is staged.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] for citation keys that hold a
/// placeholder of a user definition's template being lowered without
/// expansion, and [`LowerError`] from staging.
fn stage_calls<'t>(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'t, Flm>,
) -> Result<(Vec<BuildId>, Chunk<'t>), LowerError> {
    let mut all = Chunk::default();
    let mut out = Vec::new();
    for call in calls(node) {
        let before = all.items.len();
        read_call(&call, Reading::AtLowering, &mut all)?;
        let keys: Vec<&str> = all.items[before..].iter().map(|item| item.key.as_str()).collect();
        if keys.is_empty() {
            continue;
        }
        let note: Vec<NodeRef<'t, Flm>> = match call.argument_content_nodes_named(NOTE_ARG) {
            Ok(Some(content)) => content.iter().collect(),
            _ => Vec::new(),
        };
        let mut shapes = Vec::with_capacity(2);
        let mut slots = Vec::with_capacity(2);
        if !note.is_empty() {
            shapes.push(LatexArgShape::Optional);
            slots.push((ARG_SLOT.to_string(), lower_run(cx, lw, &note)?));
        }
        let raw = LatexRawPayload::new(keys.join(","));
        shapes.push(LatexArgShape::Mandatory);
        slots.push((ARG_SLOT.to_string(), vec![lw.pres_with(cx, &ElementName::from(RAW), raw, Vec::new())?]));
        out.push(lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(CITE_COMMAND, shapes),
            slots,
        )?);
    }
    report_mistakes(lw, &all);
    if !out.is_empty() {
        lw.require(&Name::from(PIECE));
    }
    Ok((out, all))
}

/// Stages the LaTeX hook `\<name>`, which takes no argument.
fn bare_hook(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, name: &str) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(cx, &ElementName::from(MACRO), LatexMacroPayload::new(name, []), Vec::new())
}

/// The body of [`CITE_LATEX_RULE`]: it writes `\flmCitePreSpace`, then one
/// `\flmCite` per call of the citation chunk; a dropped mark
/// (`flm.pres.dropped`) where no call has a citation key. See the [module
/// documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct CiteLatexRule;

impl Lower for CiteLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let (cites, chunk) = stage_calls(cx, lw, &node)?;
        if cites.is_empty() {
            return Ok(Lowering::Emit(vec![stage_dropped(cx, lw, &chunk, false)?]));
        }
        let mut out = vec![bare_hook(cx, lw, PRE_SPACE_COMMAND)?];
        out.extend(cites);
        Ok(Lowering::Emit(out))
    }
}

/// The body of [`REFCITE_LATEX_RULE`]: it writes `\flmRefCite` with the
/// citation counter style's word (in the variant the flags select, singular
/// or plural by how many items the citation chunk holds, a citation key
/// written twice counted twice, empty under the flag `I`) and
/// one `\flmCite` per call of the citation chunk; it writes no
/// `\flmCitePreSpace`. An `\R` with no `\cite` directly after it is reported
/// ([`NoCitation`]) and, like a citation chunk whose calls hold no citation
/// key, written as a dropped mark (`flm.pres.dropped`). Unknown flags are
/// reported as on every output ([`UnknownFlag`](super::UnknownFlag)). See the
/// [module documentation](self).
#[derive(Debug)]
pub struct RefCiteLatexRule {
    settings: Arc<MarkSettings>,
}

impl Lower for RefCiteLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let flags = read_flags(&node, lw)?;
        if leading_call(&node).is_none() {
            lw.error(NoCitation, node.span().clone());
        }
        let (cites, chunk) = stage_calls(cx, lw, &node)?;
        if cites.is_empty() {
            return Ok(Lowering::Emit(vec![stage_dropped(cx, lw, &chunk, false)?]));
        }
        let items = chunk.items.len();
        let mut word = Vec::new();
        if flags.word {
            let variant = flags.capital.then_some(CAPITAL_VARIANT);
            if let Some(text) = self.settings.style()?.word_text(items, variant) {
                word = lw.splice(cx, text)?;
            }
        }
        let hook = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(REF_CITE_COMMAND, [LatexArgShape::Mandatory, LatexArgShape::Mandatory]),
            vec![(ARG_SLOT.to_string(), word), (ARG_SLOT.to_string(), cites)],
        )?;
        Ok(Lowering::Emit(vec![hook]))
    }
}

/// The declaration of what the two rules of the recomposed LaTeX produce: the
/// LaTeX family's macro and raw constructs, and the dropped mark.
fn decl() -> LoweringDecl {
    LoweringDecl::new().produces(MACRO).produces(RAW).produces(pres::DROPPED)
}

/// The rules of `\cite` ([`CITE_LATEX_RULE`]) and of `\R`
/// ([`REFCITE_LATEX_RULE`]) on the recomposed LaTeX, marked as the defaults
/// on the `latex-recomposed` backend only.
pub(super) fn latex_rules(settings: &Arc<MarkSettings>) -> Vec<LoweringRule> {
    vec![
        LoweringRule::new(
            CITE_LATEX_RULE,
            CITE,
            decl(),
            Arc::new(CiteLatexRule),
            DefaultMark::OnBackends(vec![BACKEND.into()]),
        )
        .docs(
            "a citation chunk lowers to `\\flmCitePreSpace` and one `\\flmCite[note]{keys}` per \
             call, the keys written unescaped",
        ),
        LoweringRule::new(
            REFCITE_LATEX_RULE,
            REFCITE,
            decl(),
            Arc::new(RefCiteLatexRule { settings: Arc::clone(settings) }),
            DefaultMark::OnBackends(vec![BACKEND.into()]),
        )
        .docs(
            "a citation chunk written with `\\R` lowers to `\\flmRefCite{word}{…}` holding the \
             citation counter style's word and one `\\flmCite[note]{keys}` per call, with no \
             `\\flmCitePreSpace`",
        ),
    ]
}

/// Registers the feature's part of the recomposed LaTeX: the peer
/// declarations of the names the rules use that the LaTeX features own (the
/// backend, the macro and raw constructs), the definition piece, contributed
/// to the style file's service slot ([`preamble::SLOT`]), the rules of the
/// list of cited works ([`ListNotWritten`]) and the rules of `\cite` and
/// `\R`. The feature therefore builds in a registry without the LaTeX
/// features, with these rules set aside.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the round that takes it.
pub(super) fn register(b: &mut FeatureBuilder<'_>, settings: &Arc<MarkSettings>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the citations feature's rules of the recomposed \
         LaTeX are the defaults",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the citations feature's rules stage for \
         each LaTeX hook",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, in which the citation keys of a call are written",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    register_list(b)?;
    for rule in latex_rules(settings) {
        b.lowering_rule(rule)?;
    }
    Ok(())
}

/// The body of the rules of the list of cited works and of its entries on the
/// recomposed LaTeX: it writes nothing, because LaTeX writes its own list
/// there. See the [module documentation](self#the-list-of-cited-works).
#[derive(Debug, Default)]
pub struct ListNotWritten;

impl Lower for ListNotWritten {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// Registers [`ListNotWritten`] for the list element and its entry element,
/// marked as the default on the `latex-recomposed` backend only.
fn register_list(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let rules = [
        (BIBLIOGRAPHY, "flm.citations.bibliography.lower.latex-recomposed"),
        (BIBLIOGRAPHY_ENTRY, "flm.citations.bibliography.entry.lower.latex-recomposed"),
    ];
    for (element, rule) in rules {
        b.lowering_rule(
            LoweringRule::new(
                rule,
                element,
                LoweringDecl::new(),
                Arc::new(ListNotWritten),
                DefaultMark::OnBackends(vec![BACKEND.into()]),
            )
            .docs(
                "on the recomposed LaTeX, nothing is written for the list of the cited works: \
                 LaTeX writes its own",
            ),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_is_named_as_its_identifier_says_and_requests_no_package() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.is_empty());
        for hook in [CITE_COMMAND, PRE_SPACE_COMMAND, REF_CITE_COMMAND] {
            assert!(piece.content.contains(&format!("\\ProvideDocumentCommand\\{hook}{{")), "{hook}");
        }
        assert!(piece.content.contains("\\tl_put_right:Nn \\l_text_case_exclude_arg_tl { \\flmCite }"));
        // Nothing of a citation stands in a PDF bookmark, the word of
        // `\flmRefCite` included.
        for line in ["\\def\\flmCitePreSpace{}%", "\\let\\flmCite\\flm@cite@bookmark", "\\def\\flmRefCite#1#2{}%"] {
            assert!(piece.content.contains(line), "{line}");
        }
    }
}
