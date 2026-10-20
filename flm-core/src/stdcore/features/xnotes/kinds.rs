//! [`contribute_kind`]: the function that makes every declaration an xnote
//! kind needs, for the kinds the `xnotes` feature declares from its
//! configuration and for the kinds other features contribute.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, JoinDecl, PrefixWords, COUNTERS_SLOT, STYLES_SLOT,
};

use super::config::XnoteKind;
use super::{counter_name, list_element_name, mark_style_name, style_name, XnotesListElement, KINDS_SLOT};

/// Makes every declaration that the xnote kind `kind` needs, on behalf of the
/// feature that `b` registers.
///
/// The [`XnotesFeature`](super::XnotesFeature) calls it for every kind of its
/// configuration, and another feature calls it for a kind of its own, so that
/// both are declared alike. The function checks the kind first
/// ([`XnoteKind::validate`]).
///
/// It makes the following declarations as peer contributions, contributions
/// to a service slot another feature owns, which the registry build drops with
/// one Note when that feature is not registered (see
/// [`crate::core::services`]); the calling feature therefore builds without
/// the `xnotes` or the `numbering` feature:
///
/// - the kind itself, into the service slot of xnote kinds [`KINDS_SLOT`]
///   (`flm.xnotes.kinds`), from which the collect stage reads the kinds;
/// - for a numbered kind, into the numbering feature's service slots: the
///   counter `flm.xnotes.<kind>` ([`counter_name`]), numbered
///   [`within`](XnoteKind::within) the counter the kind states and shown by
///   default in the reference style; the reference style
///   `flm.xnotes.style.<kind>` ([`style_name`]): the kind's
///   [`words`](XnoteKind::words) and [`number_format`](XnoteKind::number_format),
///   no delimiters, several numbers joined compactly; and the mark style
///   `flm.xnotes.style.<kind>.mark` ([`mark_style_name`]): the kind's
///   [`mark`](XnoteKind::mark) template, no words, no delimiters (see
///   [counters and counter
///   styles](crate::guide::concepts_overview#counters-and-counter-styles)). An
///   unnumbered kind declares nothing to the numbering feature: its
///   [`mark`](XnoteKind::mark) holds no number placeholder, so it writes the
///   same text beside every xnote, and the `xnotes` feature compiles that text
///   once, when the registry is built, in the language of a counter style's
///   number template (the language a numbered kind's mark style is compiled
///   in); a mark that is not valid FLM fails the build.
///
/// It also makes the declarations of the kind's list, as the calling
/// feature's own:
///
/// - the synthesized list element `flm.xnotes.<kind>.list`
///   ([`list_element_name`]), a block carrying an [`XnotesListElement`], which
///   no document can write;
/// - the table `flm.xnotes.list.<kind>` ([`piece_table`](super::piece_table))
///   holding the list as a piece (a document-level [synthesized
///   value](crate::guide::concepts_overview#synthesized-values) that a
///   backend places beside the content;
///   [`Piece`](crate::core::piece::Piece)), the stage that writes it,
///   `flm.xnotes.build-list.<kind>` ([`ListBuilder`](super::ListBuilder)), and
///   the piece's declaration, contributed as a peer to the `pieces` feature's
///   service slot: placed after the content, on by default;
/// - the list's general lowering rule `flm.xnotes.<kind>.list.lower.default`
///   ([`ListRule`](super::ListRule)), and the kind's title, compiled as FLM
///   text when the registry is built (a title that is not valid FLM fails the
///   build);
/// - the list's rule on the `html` backend,
///   `flm.xnotes.<kind>.list.lower.html`
///   ([`ListHtmlRule`](super::html::ListHtmlRule)), which writes the list as
///   the general rule does, with a link back from each entry to its mark;
/// - on the LaTeX side ([`latex`](super::latex)): the kind's definition piece
///   `flm.latex.def.xnotes.<kind>`
///   ([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef),
///   a unit of LaTeX definitions that a lowering rule requires by identifier),
///   contributed as a peer to the style file's service slot; and the rule of
///   the kind's list element on the `latex-recomposed` backend, which writes
///   nothing;
/// - the peer declarations ([`PeerName`](crate::core::registry::PeerName)) of
///   every name these rules, and a rule calling
///   [`stage_latex`](super::stage_latex), use and another feature owns: the
///   table of xnotes, the numbering feature's counter table, the references
///   feature's anchors and safe-labels tables, the presentation elements, the
///   `html` and `latex-recomposed` backends and the LaTeX family's macro and
///   raw constructs. The calling feature therefore builds and renders without
///   any of those features: a rule whose backend or produced element nobody
///   declares is set aside (excluded from rule selection; see
///   [`PeerName`](crate::core::registry::PeerName)), and a rule that reads a
///   table optionally finds it absent. Where the `xnotes` feature is absent,
///   the document renders with no list placed. A feature may call the function
///   for several kinds, or declare some of these names itself: the registry
///   merges a repeated identical peer declaration into the first.
///
/// The function declares no command: an author writes xnotes only with the
/// commands the `xnotes` feature declares for the kinds of its own
/// configuration. A kind another feature creates has no command
/// (`commands: []`); its xnotes are that feature's own nodes, whose element
/// implements the [`XnoteSource`](super::XnoteSource) aspect and returns its
/// reference points with [`ref_points`](super::ref_points). For the same
/// reason the function declares no peer of the references feature's scoped
/// package of labels
/// ([`refs::LABEL_PACKAGE`](crate::stdcore::features::refs::LABEL_PACKAGE)):
/// a feature that registers an element whose argument refers to that package
/// declares the package as a peer itself
/// ([`FeatureBuilder::peer_scoped_package`]).
///
/// Two features that contribute kinds of one name fail the registry build:
/// both declare the list element of that name, and, for a numbered kind, the
/// counter of that name.
///
/// # Errors
///
/// [`BuildError::Feature`] when the kind fails [`XnoteKind::validate`]; the
/// [`BuildError`] of the registry builder when a declaration is made in a
/// [registration round](crate::core::registry::builder#the-two-registration-rounds)
/// that does not accept it (from a finalizer, for instance).
pub fn contribute_kind(b: &mut FeatureBuilder<'_>, kind: Arc<XnoteKind>) -> Result<(), BuildError> {
    let owner = b.feature().clone();
    kind.validate().map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
    b.peer_slot::<XnoteKind>(KINDS_SLOT, Arc::clone(&kind))?;
    if kind.numbered {
        b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(reference_style(&kind)))?;
        b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(mark_style(&kind)))?;
        b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter(&kind)))?;
    }
    let list = super::list::declare_list(b, &kind)?;
    super::html::declare_kind(b, &kind, list)?;
    super::latex::declare_kind(b, &kind)?;
    let name = kind.name.clone();
    b.element(
        ElementDef::new(owner, Vec::<&str>::new(), SyntaxDef::Synthesized, list_element(kind))
            .docs(format!(
                "The list of the `{name}` xnotes of a document. It is never written in a \
                 document: the kind's list builder creates it."
            )),
    )?;
    Ok(())
}

/// The counter of the numbered xnote kind `kind`: shown in the reference
/// style, numbered within the counter the kind states, with no nesting
/// joiner.
fn counter(kind: &XnoteKind) -> CounterDecl {
    let name = kind.name.as_str();
    let decl = CounterDecl::new(counter_name(name), style_name(name))
        .docs(format!("The `{name}` xnotes of the document, in order of appearance."));
    match &kind.within {
        Some(within) => decl.within(within.clone()),
        None => decl,
    }
}

/// The reference style of the numbered xnote kind `kind`: its words, its
/// number format, no delimiters, several numbers joined compactly.
fn reference_style(kind: &XnoteKind) -> CounterStyleDecl {
    // Several numbers join compactly, as in every standard feature's styles.
    // See decision rationale at [§flm-dd-dr:std-xnote-reference-join].
    let mut style = CounterStyleDecl::new(style_name(kind.name.as_str()));
    style.number_format = kind.number_format.clone();
    style.prefix_words = kind.words.clone();
    style.join = JoinDecl::compact();
    style
}

/// The mark style of the xnote kind `kind`: its mark template, no words, no
/// delimiters. A numbered kind declares it to the numbering feature; the
/// feature itself compiles an unnumbered kind's fixed mark as this style
/// (the private module `marks`), so that one template is read in one way.
pub(super) fn mark_style(kind: &XnoteKind) -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(mark_style_name(kind.name.as_str()));
    style.number_format = kind.mark.clone();
    style.prefix_words = PrefixWords::default();
    style
}

/// The element value of the list of the xnote kind `kind`: a block carrying
/// the kind.
fn list_element(kind: Arc<XnoteKind>) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(list_element_name(kind.name.as_str()))
            .docs(format!("The list of the `{}` xnotes of a document.", kind.name))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with_value(Arc::new(XnotesListElement { kind })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::numbering::{NumberTemplate, Within};

    #[test]
    fn a_numbered_kind_declares_its_counter_and_its_two_styles() {
        let kind = XnoteKind::footnote().within(Within::parent("flm.headings.section"));
        let counter = counter(&kind);
        assert_eq!(counter.name.as_str(), "flm.xnotes.footnote");
        assert_eq!(counter.default_style.as_str(), "flm.xnotes.style.footnote");
        assert!(counter.within.is_some());
        assert!(counter.nesting.is_none());
        let reference = reference_style(&kind);
        assert_eq!(reference.name.as_str(), "flm.xnotes.style.footnote");
        assert_eq!(reference.prefix_words, kind.words);
        assert_eq!(reference.number_format, NumberTemplate::formats("#{arabic}"));
        assert_eq!(reference.delimiters.open, "");
        assert_eq!(reference.join, JoinDecl::compact());
        let mark = mark_style(&kind.mark(NumberTemplate::formats("[#{alph}]")));
        assert_eq!(mark.name.as_str(), "flm.xnotes.style.footnote.mark");
        assert_eq!(mark.number_format, NumberTemplate::formats("[#{alph}]"));
        assert_eq!(mark.prefix_words, PrefixWords::default());
        assert_eq!(mark.delimiters.close, "");
    }
}
