//! The [`xnotes`](super) feature on the `latex-rendered` backend: its
//! definition piece, `flm.latex.def.flmx.xnotes` ([`piece`]), and the one
//! rule of the standard features written for this backend alone, the
//! xnote's rule [`RULE`] ([`XnoteRenderedRule`]).
//!
//! # The rule
//!
//! An xnote kind's `placement` decides how its xnotes are written, as on
//! every backend. For a kind placed in a list, the rule stages exactly what
//! the general rule stages: the mark, through
//! [`stage_mark`](super::stage_mark), a superscript span of role
//! `flm.xnotes.xnote.mark` holding a link of kind `xnote` to the xnote's
//! entry in the kind's list. For a kind placed in place, it stages the macro
//! construct `\flmxXnote` with three arguments, so that LaTeX receives the
//! mark and the content apart (LaTeX's `\footnote` takes them apart):
//!
//! ```latex
//! \flmxXnote[kind=footnote, anchor=footnote-3, doc-role=footnote]{<mark>}{<content>}
//! ```
//!
//! The optional argument is a raw node holding the keys: the kind's name,
//! the xnote's anchor where the plan computes one, and the document role.
//! The first mandatory argument is the mark, the span of role
//! `flm.xnotes.xnote.mark` carrying the mark's anchor and holding the mark
//! text, with no link (the content is beside it); the second is the xnote's
//! content lowered through the rules, as blocks where it holds paragraphs.
//! The brackets the general rule writes around an xnote in place are not
//! written: the LaTeX definition decides the look. The rule reports what the
//! general rule reports: the refused content of the optional argument, an
//! empty xnote and a missing entry.
//!
//! # The piece
//!
//! The piece is the LaTeX text of the fragment `sty/xnotes.sty`, included at
//! compile time. It defines `\flmxXnote`, which dispatches on the key `kind`
//! (`\flmxDefineXnote{<kind>}{…}`) and whose default sets a footnote at the
//! bottom of the page carrying FLM's mark (not LaTeX's footnote counter),
//! the note's text beginning with the target of its anchor. It registers
//! the block of every kind's list under the kind token `xnotes` (a block of
//! paragraphs, the target of its anchor first) and the link kinds `xnote`
//! and `xnote-back` as the links chunk's default link. The marks' spans are
//! left to the span's default, the superscript of their style. Its comment
//! block says how a template registers another look.
//!
//! It delivers the feature's style identifier [`STYLE`](super::STYLE),
//! which the feature's rules require at every xnote and every list: a
//! `latex-rendered` document holding an xnote holds the piece in its style
//! file, and one that does not, does not.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) and registers the rule under peer declarations of
//! the backend and of the construct elements, so that a registry without
//! the LaTeX backends still builds.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::preamble::{self, LatexPreambleDef};
use crate::stdcore::backends::latex::{LatexArgShape, LatexMacroPayload, LatexRawPayload};
use crate::stdcore::backends::latex_rendered::BACKEND;
use crate::stdcore::backends::latex_rendered::contract::{CORE_CHUNK, LINKS_CHUNK};
use crate::stdcore::backends::latex_rendered::emit::{anchor_spelling, is_bare_name};
use crate::stdcore::backends::lowering::{lower_blocks_with_edges, lower_run};

use super::config::{XnoteKind, XnotePlacement};
use super::lower::{
    Brackets, Content, Mark, find, identifiers, lower_xnote, report_empty, report_labels,
    report_missing, write_mark,
};
use super::{XNOTE, XnoteElement, xnote_decl};

/// The piece's identifier, `flm.latex.def.flmx.xnotes`.
pub const PIECE: &str = "flm.latex.def.flmx.xnotes";

/// The name of the xnote's rule on the `latex-rendered` backend,
/// `flm.xnotes.xnote.lower.latex-rendered` ([`XnoteRenderedRule`]).
pub const RULE: &str = "flm.xnotes.xnote.lower.latex-rendered";

/// The LaTeX command, without the backslash, that the rule writes for an
/// xnote of a kind placed in place, `flmxXnote`.
pub const COMMAND: &str = "flmxXnote";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/xnotes.sty`, requiring the links chunk, whose default
/// link its link kinds use. See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.xnotes",
        include_str!("sty/xnotes.sty"),
        "notes: `\\flmxXnote`, the lists of notes and the links of notes",
    )
    .requires([CORE_CHUNK, LINKS_CHUNK])
    .delivering([super::STYLE])
}

/// The xnote's rule on the `latex-rendered` backend, [`RULE`]: for a kind
/// placed in a list, the general rule's staging (the mark); for a kind
/// placed in place, the construct `\flmxXnote`. See the
/// [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct XnoteRenderedRule;

impl Lower for XnoteRenderedRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<XnoteElement>(&node)
            .ok_or_else(|| LowerError::structure("an xnote without its element's data"))?;
        let kind = Arc::clone(&value.kind);
        if matches!(kind.placement, XnotePlacement::List) {
            // Exactly the general rule, so that the two cannot differ.
            return lower_xnote(&node, cx, lw, Brackets::Text);
        }
        lw.require(&Name::from(super::STYLE));
        report_labels(lw, &node, &kind);
        // The element carries one xnote, the first of its kind.
        Ok(Lowering::Emit(stage_rendered(cx, lw, &node, &kind, 0)?))
    }
}

/// Stages the `nth` xnote of the kind `kind` that `node` carries as the
/// construct `\flmxXnote[<keys>]{<mark>}{<content>}`: the keys as a raw
/// node, the mark as [`write_mark`] stages it (anchored, not linked), the
/// content lowered through the rules, as blocks where it holds paragraphs.
/// An xnote the table does not record is written as nothing and reported as
/// `flm.run.missing-entry`; an empty one is reported as
/// [`Empty`](super::Empty).
///
/// # Errors
///
/// [`LowerError`] when `node` is not a node of the tree being lowered, when
/// the calling rule did not declare what it reads or produces, and from the
/// lowering of the content.
fn stage_rendered(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(record) = find(lw, node, kind, nth)? else {
        report_missing(lw, node);
        return Ok(Vec::new());
    };
    report_empty(lw, node, kind, nth)?;
    let (_, anchor) = identifiers(lw, &record)?;
    let keys = keys(kind.name.as_str(), anchor.as_deref());
    let keys = lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(keys), Vec::new())?;
    let mark = write_mark(cx, lw, kind, &record, Mark { anchored: true, linked: false })?;
    let content = Content::of(node, kind, nth);
    let body = if content.holds_blocks(node) {
        lower_blocks_with_edges(cx, lw, content.nodes(), Vec::new(), Vec::new())?
    } else {
        lower_run(cx, lw, content.nodes())?
    };
    let payload = LatexMacroPayload::new(
        COMMAND,
        vec![LatexArgShape::Optional, LatexArgShape::Mandatory, LatexArgShape::Mandatory],
    );
    let slots = vec![
        (ARG_SLOT.to_string(), vec![keys]),
        (ARG_SLOT.to_string(), vec![mark]),
        (ARG_SLOT.to_string(), body),
    ];
    Ok(vec![lw.pres_slots_with(cx, &ElementName::from(MACRO), payload, slots)?])
}

/// The keys of `\flmxXnote`, written as the backend writes an element's
/// keys: `kind=<kind>, anchor=<anchor>, doc-role=footnote`, the anchor left
/// out where there is none, bare where it may stand bare
/// ([`is_bare_name`]) and otherwise braced and spelled as an address
/// ([`anchor_spelling`]), so that it reads back as itself.
fn keys(kind: &str, anchor: Option<&str>) -> String {
    let mut keys = format!("kind={kind}");
    if let Some(anchor) = anchor {
        if is_bare_name(anchor) {
            keys.push_str(&format!(", anchor={anchor}"));
        } else {
            keys.push_str(&format!(", anchor={{{}}}", anchor_spelling(anchor)));
        }
    }
    keys.push_str(", doc-role=footnote");
    keys
}

/// The xnote's rule on the `latex-rendered` backend, [`RULE`]
/// ([`XnoteRenderedRule`]), marked default on that backend only.
pub fn rule() -> LoweringRule {
    LoweringRule::new(
        RULE,
        XNOTE,
        xnote_decl().produces(MACRO).produces(RAW),
        Arc::new(XnoteRenderedRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "an xnote lowers as under the general rule for a listed xnote kind; for an in-place \
         kind, to the macro construct `\\flmxXnote[<keys>]{<mark>}{<content>}`, its keys as a \
         raw node, its mark unlinked, its content lowered through the rules",
    )
}

/// Contributes [`piece`] to the preamble slot, and, where the configuration
/// declares an xnote kind with a command (where the element [`XNOTE`]
/// exists), registers [`rule`], under peer declarations of the backend and
/// of the macro and raw construct elements.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the registration round that accepts it.
pub(super) fn register(b: &mut FeatureBuilder<'_>, with_element: bool) -> Result<(), BuildError> {
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece()))?;
    if !with_element {
        return Ok(());
    }
    b.peer_backend(
        BACKEND,
        "the latex-rendered backend, on which the xnote's rule for this backend is the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the rule stages for `\\flmxXnote`",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, which holds the keys of `\\flmxXnote`",
    )?;
    b.lowering_rule(rule())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_piece_names_itself_and_requires_the_links_chunk() {
        let piece = piece();
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.iter().any(|id| id.as_str() == LINKS_CHUNK));
        assert_eq!(piece.delivers, [Name::from(super::super::STYLE)]);
        assert!(piece.content.starts_with(&format!("% {PIECE} -- ")), "{}", piece.content);
    }

    #[test]
    fn the_keys_are_spelled_as_the_backend_spells_an_element_s_keys() {
        assert_eq!(
            keys("footnote", Some("footnote-3")),
            "kind=footnote, anchor=footnote-3, doc-role=footnote"
        );
        assert_eq!(keys("footnote", None), "kind=footnote, doc-role=footnote");
        assert_eq!(
            keys("note", Some("a#b~c")),
            "kind=note, anchor={a\\#b\\string~c}, doc-role=footnote"
        );
    }
}
