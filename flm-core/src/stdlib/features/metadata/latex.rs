//! The lowering rules of the metadata's pieces on the `latex-recomposed` backend: the
//! title, the authors and the date written back as the LaTeX text that a LaTeX
//! document passes to `\title`, `\author` and `\date`.
//!
//! - The title and the date are their content, written back as LaTeX
//!   ([`MetaTextLatexRule`]): `On \emph{flexible} markup`.
//! - The authors are each author as a LaTeX author types one in `\author` with the
//!   standard classes, the authors joined by LaTeX's `\and` ([`AuthorLatexRule`]).
//!   An author ([`EntryLatexRule`]) is the name, then `\thanks{…}` for each note,
//!   then each affiliation and the email address, each on a line of its own after
//!   `\\`:
//!
//!   ```latex
//!   Ada Lovelace\thanks{Corresponding author} \\ University of X \\ ada@example.com \and Grace Hopper
//!   ```
//!
//!   The name and an affiliation are their content, written back as LaTeX
//!   ([`MetaTextLatexRule`]); a note is `\thanks` around its content
//!   ([`NoteLatexRule`]); the email address is its text ([`EmailLatexRule`]).
//!
//! The rules are the defaults on the `latex-recomposed` backend only. That backend
//! places a piece only where the render options of the `pieces` feature list it, and
//! leaves the piece out of its entry artifact (the file that stands for the output;
//! [`ArtifactSet`](crate::core::artifact::ArtifactSet)) for the program to place ([pieces in the
//! recomposed LaTeX](crate::guide::rendering#pieces-in-the-recomposed-latex)).
//!
//! The feature registers the rules with a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of the backend and of the macro
//! construct `flm.pres.latex.macro` the rules of the authors stage, so that a registry
//! without the LaTeX features still builds, with the rules set aside (excluded from
//! rule selection).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::ElementName;
use crate::stdlib::backends::latex::construct::{ARG_SLOT, MACRO};
use crate::stdlib::backends::latex::{LatexArgShape, LatexMacroPayload};
use crate::stdlib::backends::latex_recomposed::BACKEND;

use super::rules::{each_child, email_of, inline_content};
use super::{AUTHOR, AUTHOR_AFFILIATION, AUTHOR_EMAIL, AUTHOR_ENTRY, AUTHOR_NAME, AUTHOR_NOTE, DATE, TITLE};

/// The command that ends a line of an author, `\\`, without its backslash.
const LINE_BREAK: &str = "\\";

/// The command around a note about an author, `\thanks`, without its backslash.
const THANKS: &str = "thanks";

/// Returns the name of the lowering rule of the element `element` on the
/// `latex-recomposed` backend, `<element>.lower.latex-recomposed`.
pub fn latex_rule_name(element: &str) -> String {
    format!("{element}.lower.latex-recomposed")
}

/// Returns a call of the command `name` with no argument.
fn command(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, name: &str) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(cx, &ElementName::from(MACRO), LatexMacroPayload::new(name, []), Vec::new())
}

/// The lowering rule of the title, of the date, of an author's name and of an
/// affiliation on the `latex-recomposed` backend: the content, written back as
/// LaTeX.
#[derive(Debug, Default)]
pub struct MetaTextLatexRule;

impl Lower for MetaTextLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(inline_content(&node, cx, lw)?))
    }
}

/// The lowering rule of the authors on the `latex-recomposed` backend: each
/// author, lowered through its rule ([`EntryLatexRule`]), the authors joined by
/// ` \and `.
#[derive(Debug, Default)]
pub struct AuthorLatexRule;

impl Lower for AuthorLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut joined = Vec::new();
        for one in each_child(&node)? {
            if !joined.is_empty() {
                joined.push(lw.text(cx, " ")?);
                joined.push(command(cx, lw, "and")?);
                joined.push(lw.text(cx, " ")?);
            }
            joined.extend(lw.lower_nodes(cx, one)?);
        }
        Ok(Lowering::Emit(joined))
    }
}

/// The lowering rule of one author on the `latex-recomposed` backend: the name,
/// then each note, then each affiliation and the email address after ` \\ `, each
/// part lowered through its rule.
#[derive(Debug, Default)]
pub struct EntryLatexRule;

impl Lower for EntryLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut name = Vec::new();
        let mut notes = Vec::new();
        let mut lines = Vec::new();
        for (one, child) in each_child(&node)?.into_iter().zip(node.children().iter()) {
            let part = lw.lower_nodes(cx, one)?;
            match child.ext().element.name.as_str() {
                AUTHOR_NOTE => notes.extend(part),
                AUTHOR_AFFILIATION | AUTHOR_EMAIL => lines.push(part),
                _ => name.extend(part),
            }
        }
        let mut written = name;
        written.extend(notes);
        for line in lines {
            written.push(lw.text(cx, " ")?);
            written.push(command(cx, lw, LINE_BREAK)?);
            written.push(lw.text(cx, " ")?);
            written.extend(line);
        }
        Ok(Lowering::Emit(written))
    }
}

/// The lowering rule of a note about an author on the `latex-recomposed` backend:
/// `\thanks{…}` around the note's content, written back as LaTeX.
#[derive(Debug, Default)]
pub struct NoteLatexRule;

impl Lower for NoteLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let content = inline_content(&node, cx, lw)?;
        let thanks = LatexMacroPayload::new(THANKS, [LatexArgShape::Mandatory]);
        let call = lw.pres_slots_with(cx, &ElementName::from(MACRO), thanks, vec![(String::from(ARG_SLOT), content)])?;
        Ok(Lowering::Emit(vec![call]))
    }
}

/// The lowering rule of an author's email address on the `latex-recomposed`
/// backend: the address, as text.
#[derive(Debug, Default)]
pub struct EmailLatexRule;

impl Lower for EmailLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let address = email_of(&node)?;
        Ok(Lowering::Emit(vec![lw.text(cx, address)?]))
    }
}

/// Registers the rules, with the peer declarations of the backend and of the macro
/// construct they need.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the metadata's LaTeX rules are the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the rules of the authors stage for `\\and`, `\\\\` \
         and `\\thanks`",
    )?;
    let on_backend = || DefaultMark::OnBackends(vec![BACKEND.into()]);
    for element in [TITLE, DATE, AUTHOR_NAME, AUTHOR_AFFILIATION] {
        b.lowering_rule(
            LoweringRule::new(
                latex_rule_name(element),
                element,
                LoweringDecl::new(),
                Arc::new(MetaTextLatexRule),
                on_backend(),
            )
            .docs(format!("on the recomposed LaTeX, `{element}` is its content, written back as LaTeX")),
        )?;
    }
    let rules: [(&str, Arc<dyn Lower>, &str); 4] = [
        (AUTHOR, Arc::new(AuthorLatexRule), "on the recomposed LaTeX, the authors are each author, joined by `\\and`"),
        (
            AUTHOR_ENTRY,
            Arc::new(EntryLatexRule),
            "on the recomposed LaTeX, an author is the name, each note, then each affiliation and the email \
             address after `\\\\`",
        ),
        (AUTHOR_NOTE, Arc::new(NoteLatexRule), "on the recomposed LaTeX, a note is `\\thanks` around its content"),
        (AUTHOR_EMAIL, Arc::new(EmailLatexRule), "on the recomposed LaTeX, an email address is its text"),
    ];
    for (element, rule, docs) in rules {
        let decl = if element == AUTHOR_EMAIL { LoweringDecl::new() } else { LoweringDecl::new().produces(MACRO) };
        b.lowering_rule(LoweringRule::new(latex_rule_name(element), element, decl, rule, on_backend()).docs(docs))?;
    }
    Ok(())
}
