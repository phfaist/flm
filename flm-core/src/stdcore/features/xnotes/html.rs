//! The lowering rules of the [`xnotes`](super) feature for the `html`
//! backend, and the feature's stylesheet.
//!
//! On the HTML page, an xnote and a list of xnotes are written with the same
//! presentation elements as on every other output: the rules of this module
//! call the staging helpers of the general rules
//! ([`stage_mark`](super::stage_mark), [`stage_in_place`](super::stage_in_place),
//! [`ListRule`]), so that the page writes the same structure as the other
//! outputs that use these shapes, and
//! the HTML backend writes those elements as its HTML contract states (the
//! classes and attributes a stylesheet may rely on; see
//! [`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS) and [the HTML
//! contract](crate::guide::rendering#the-html-contract)). The rules of this
//! module are marked default on the `html` backend and add what only the page
//! needs:
//!
//! - the **link back** from each entry of a list of xnotes to its mark
//!   ([`ListHtmlRule`]);
//! - **no bracket text** around an xnote shown in place ([`XnoteHtmlRule`]):
//!   the stylesheet draws the brackets, so that a page's own stylesheet can
//!   replace them;
//! - the **stylesheet** [`STYLE`] (`flm.xnotes.style`). The staging helpers
//!   that the general rules and these rules share require the feature's
//!   style identifier [`STYLE`] at every xnote and every list: they record it
//!   as a requirement of the output, and on this backend it names the
//!   stylesheet, which the backend then writes or loads (see [`StyleDef`]).
//!
//! # The markup
//!
//! The worked example, `The claim holds in every case.\footnote{Except the
//! empty one, which we exclude by convention.} The next sentence goes on.`,
//! writes the mark as a superscript carrying the mark's anchor (its `id`),
//! around a link to the xnote. The link carries the document role
//! `doc-noteref` and an accessible name (the name that assistive technology
//! reads in place of the link's text), so that a screen reader announces
//! "Footnote 1" rather than "1":
//!
//! ```html
//! <p class="flm-pres-par">The claim holds in every case.<sup class="flm-pres-span flm-xnotes-xnote-mark" id="fnref--1yuhk" data-flm-style="superscript"><a class="flm-pres-link" href="#fn-1" role="doc-noteref" aria-label="Footnote 1" data-flm-kind="xnote">1</a></sup> The next sentence goes on.</p>
//! ```
//!
//! The list of the xnote kind is placed after the document's content: a block
//! of the document role `doc-endnotes` holding the title as a heading and a
//! labeled list whose entries carry the xnotes' anchors. The last paragraph of
//! each entry ends with a link back to the mark, of the role `doc-backlink`,
//! whose text is the kind's [`backlink`](super::XnoteKind::backlink) (`↩`) and
//! whose accessible name is `Back to ` followed by the xnote's name as a
//! reference writes it:
//!
//! ```html
//! <div class="flm-pres-block flm-xnotes-footnote-list" role="doc-endnotes" data-flm-kind="xnotes footnote">
//!   <h2 class="flm-pres-block--heading" data-flm-heading="xnotes" data-flm-level="2" data-flm-toc="false">Footnotes</h2>
//!   <ol class="flm-pres-list" data-flm-kind="numbered xnotes footnote labeled">
//!     <li id="fn-1"><span class="flm-pres-list-label"><sup class="flm-pres-span flm-xnotes-entry-mark" data-flm-style="superscript">1</sup></span>
//!       <p class="flm-pres-par">Except the empty one, which we exclude by convention. <a class="flm-pres-link" href="#fnref--1yuhk" role="doc-backlink" aria-label="Back to Footnote 1" data-flm-kind="xnote-back">↩</a></p>
//!     </li>
//!   </ol>
//! </div>
//! ```
//!
//! The heading is written with the tag of its level, or with the tag to which
//! the render option
//! [`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags)
//! maps the heading name `xnotes` ([`TITLE_HEADING`](super::TITLE_HEADING)).
//! Where the plan computes no anchors (in a registry without the references
//! feature), the mark has no link and the entry no link back.
//!
//! An xnote of a kind shown **in place** is one span where its content is
//! inline, carrying the xnote's anchor, its kind tokens and the document role
//! `doc-footnote`, with the mark inside it and no link:
//!
//! ```html
//! …every case. <span class="flm-pres-span flm-xnotes-xnote-inplace" id="fn-1" role="doc-footnote" data-flm-kind="xnotes footnote"><sup class="flm-pres-span flm-xnotes-xnote-mark" id="fnref--1yuhk" data-flm-style="superscript">1</sup>Except the empty one, which we exclude by convention.</span> The next…
//! ```
//!
//! Where its content holds paragraphs, it is a block between the two parts of
//! the paragraph it interrupts, with its mark at the head of its first
//! paragraph:
//!
//! ```html
//! <p class="flm-pres-par">Two paragraphs.</p>
//! <div class="flm-pres-block flm-xnotes-xnote-inplace" id="fn-1" role="doc-footnote" data-flm-kind="xnotes footnote">
//!   <p class="flm-pres-par"><sup class="flm-pres-span flm-xnotes-xnote-mark" id="fnref--1yuhk" data-flm-style="superscript">1</sup>First paragraph.</p>
//!   <p class="flm-pres-par">Second paragraph.</p>
//! </div>
//! <p class="flm-pres-par">And on.</p>
//! ```
//!
//! The mark of an unnumbered xnote kind is its fixed mark, and an empty fixed
//! mark gives an empty `<sup>`, so that the mark's anchor still exists.
//!
//! # The stylesheet
//!
//! The style definition [`stylesheet`] (`flm.xnotes.style`; see
//! [`StyleDef`]) is one cascade layer holding the line above a list of xnotes
//! and the brackets around an xnote shown in place, and nothing more. Every
//! value it uses is a custom property declared in it
//! (`--flm-xnotes-list-rule`, `--flm-xnotes-list-size`,
//! `--flm-xnotes-list-space`, `--flm-xnotes-inplace-open`,
//! `--flm-xnotes-inplace-close`), which a page sets to change these values
//! without writing a rule of its own. The xnote and the list require it, so a
//! page holding no xnote does not carry it.
//!
//! **The feature ships no script.** An xnote shown in place is always written
//! whole, with every anchor, role and kind token as an attribute, so that a
//! page's own stylesheet or script can lay it out, hide it or show it; a
//! script shipped by the feature would interfere with the page's own. A host
//! adds a stylesheet or a script of its own by declaring a style or script
//! definition in a feature of its own
//! ([`FeatureBuilder::style_def`](crate::core::registry::FeatureBuilder::style_def),
//! [`script_def`](crate::core::registry::FeatureBuilder::script_def)) and
//! requiring it from a rule, or replaces the content of this stylesheet
//! through the render option
//! [`definitions`](crate::stdcore::backends::html::HtmlRenderOptions::definitions).
//!
//! # Registration
//!
//! The rules are marked default on the `html` backend only, and are
//! registered under a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of that backend, so that a
//! registry without the backend still builds, with the rules set aside
//! (excluded from rule selection). The `xnotes` feature registers the
//! stylesheet and, where a configured xnote kind has a command, the xnote's
//! rule ([`html_rule`]);
//! [`contribute_kind`](super::contribute_kind) registers the list's rule for
//! each xnote kind, like every rule of a kind's list.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringRule,
};
use crate::core::registry::{BuildError, DefContent, FeatureBuilder, StyleDef};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name, RuleName};
use crate::stdcore::backends::html::BACKEND;
use crate::stdcore::backends::pres::{self, DocRole, LinkPayload};
use crate::stdcore::features::refs::ANCHORS;

use super::config::XnoteKind;
use super::list::{list_decl, ListRule};
use super::lower::{accessible_name, lower_xnote, Brackets};
use super::stages::XnoteRecord;
use super::{list_element_name, xnote_decl, BACKLINK_KIND, STYLE, XNOTE};

/// The name of the lowering rule of an xnote on the `html` backend,
/// `flm.xnotes.xnote.lower.html` ([`XnoteHtmlRule`]).
pub const HTML_RULE: &str = "flm.xnotes.xnote.lower.html";

/// The words with which the accessible name of an entry's link back to its
/// mark begins, before the xnote's name: `Back to`.
pub const BACKLINK_LABEL_PREFIX: &str = "Back to";

/// The name of the lowering rule of the list element of the xnote kind `kind`
/// on the `html` backend, `flm.xnotes.<kind>.list.lower.html`
/// ([`ListHtmlRule`]).
pub fn list_rule_name(kind: &str) -> RuleName {
    RuleName::from(format!("{}.lower.html", list_element_name(kind)))
}

/// The feature's style definition, `flm.xnotes.style` ([`STYLE`]): the line
/// above a list of xnotes and the brackets around an xnote shown in place,
/// in one cascade layer, with every value a custom property it declares. The
/// `xnotes` feature registers it; the xnote and the list require it.
/// See the [module documentation](self).
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/xnotes.css")))],
        docs: String::from(
            "the line above a list of xnotes and the brackets around an xnote shown in place",
        ),
    }
}

/// The lowering rule of an xnote on the `html` backend, [`HTML_RULE`]: the
/// xnote's mark, or the xnote in place, as the general rule writes them
/// ([`stage_mark`](super::stage_mark),
/// [`stage_in_place`](super::stage_in_place)), but with no bracket text,
/// because the stylesheet draws the brackets. It requires the style
/// identifier [`STYLE`] and reports the refused content of the optional
/// argument as the general rule does.
#[derive(Debug, Default, Clone, Copy)]
pub struct XnoteHtmlRule;

impl Lower for XnoteHtmlRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lower_xnote(&node, cx, lw, Brackets::Drawn)
    }
}

/// The lowering rule of an xnote kind's list element on the `html` backend
/// ([`list_rule_name`]): the list as the general rule writes it
/// ([`ListRule`]), with the link back to each xnote's mark at the end of the
/// last paragraph of its entry. It requires the style identifier [`STYLE`],
/// as the general rule does.
///
/// The link back is
/// `flm.pres.link { href: "#<the mark's anchor>", kind: xnote-back,
/// doc_role: BackLink, label: "Back to <the xnote's name>" }` holding the
/// kind's [`backlink`](XnoteKind::backlink) text, after a space. The xnote's
/// name is the kind's singular word, as a reference writes it in running text
/// (with no variant), then the xnote's number as a reference writes it
/// without the word (`Back to Footnote 1`); the word alone where there is no
/// number. There is no link back where the mark has no anchor (in a registry
/// without the references feature), for an entry whose pointer refers to no
/// recorded xnote, and for a kind whose `backlink` text is empty.
#[derive(Debug)]
pub struct ListHtmlRule {
    kind: Arc<XnoteKind>,
    list: Arc<ListRule>,
}

impl Lower for ListHtmlRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let kind = &self.kind;
        self.list.lower_list(&node, cx, lw, &mut |cx, lw, record| backlink(cx, lw, kind, record))
    }
}

/// Stages the link back from the entry of the xnote `record` of the xnote
/// kind `kind` to its mark, after a space, as [`ListHtmlRule`] documents it;
/// nothing where there is no link back.
fn backlink(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    kind: &XnoteKind,
    record: Option<&XnoteRecord>,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(record) = record else { return Ok(Vec::new()) };
    if kind.backlink.is_empty() {
        return Ok(Vec::new());
    }
    let mark = match lw.get(ANCHORS)? {
        Some(anchors) => anchors.lookup(record.mark).flatten().map(String::from),
        None => None,
    };
    let Some(mark) = mark else { return Ok(Vec::new()) };
    // The word a reference writes in running text: no variant.
    // See decision rationale at [§flm-dd-dr:std-xnote-backlink].
    let name = accessible_name(lw, kind, &kind.words.base.one, record.point)?;
    let label = if name.is_empty() {
        String::from(BACKLINK_LABEL_PREFIX)
    } else {
        format!("{BACKLINK_LABEL_PREFIX} {name}")
    };
    let link = LinkPayload {
        href: format!("#{mark}"),
        kind: Name::from(BACKLINK_KIND),
        doc_role: Some(DocRole::BackLink),
        label: Some(label),
        shows_target: false,
    };
    let text = lw.text(cx, &kind.backlink)?;
    let space = lw.text(cx, " ")?;
    Ok(vec![space, lw.pres_with(cx, &ElementName::from(pres::LINK), link, vec![text])?])
}

/// The lowering rule of an xnote on the `html` backend, [`HTML_RULE`]
/// ([`XnoteHtmlRule`]), marked default on that backend only.
pub fn html_rule() -> LoweringRule {
    LoweringRule::new(HTML_RULE, XNOTE, xnote_decl(), Arc::new(XnoteHtmlRule), on_html())
        .docs(
            "an xnote lowers as under the general rule, with no bracket text around an xnote \
             shown in place: the stylesheet `flm.xnotes.style`, which the rule requires, draws \
             the brackets",
        )
}

/// The lowering rule of the list element of the xnote kind `kind` on the
/// `html` backend ([`ListHtmlRule`], [`list_rule_name`]), marked default on
/// that backend only, writing the list as `list` does, with the links back.
fn list_html_rule(kind: &Arc<XnoteKind>, list: Arc<ListRule>) -> LoweringRule {
    let name = kind.name.as_str();
    LoweringRule::new(
        list_rule_name(name),
        list_element_name(name),
        list_decl().produces(pres::LINK),
        Arc::new(ListHtmlRule { kind: Arc::clone(kind), list }),
        on_html(),
    )
    .docs(
        "the list of a kind's xnotes lowers as under the general rule, each entry's last \
         paragraph ending with a link back to the xnote's mark; it requires the stylesheet \
         `flm.xnotes.style`",
    )
}

/// The default mark of every rule of this module: default on the `html`
/// backend only.
fn on_html() -> DefaultMark {
    DefaultMark::OnBackends(vec![BACKEND.into()])
}

/// Makes a peer declaration of the `html` backend, on behalf of the feature
/// that `b` registers, so that a registry without the backend builds with the
/// rules of this module set aside. Every call declares it: the registry
/// merges an identical repeated peer declaration into the first.
fn declare_backend(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(BACKEND, "the html backend, on which the HTML rules of xnotes are the default")
}

/// Registers what the `xnotes` feature itself contributes to the page: the
/// stylesheet ([`stylesheet`]), and, where the configuration declares an
/// xnote kind with a command (where the element [`XNOTE`] exists), the
/// xnote's rule ([`html_rule`]) under the peer declaration of the backend.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// in a [registration round](crate::core::registry::builder#the-two-registration-rounds)
/// that does not accept it (from a finalizer, for instance).
pub(super) fn register(b: &mut FeatureBuilder<'_>, with_element: bool) -> Result<(), BuildError> {
    // Declared once, by this feature, never per kind.
    // See decision rationale at [§flm-dd-dr:std-xnote-stylesheet].
    b.style_def(stylesheet())?;
    if with_element {
        declare_backend(b)?;
        b.lowering_rule(html_rule())?;
    }
    Ok(())
}

/// Declares what the xnote kind `kind` needs on the page, on behalf of the
/// feature that `b` registers: the rule of its list element on the `html`
/// backend, which writes the list as `list` (the kind's general rule) does,
/// under the peer declaration of the backend.
/// [`contribute_kind`](super::contribute_kind) calls it once per kind.
///
/// Only the `xnotes` feature declares the stylesheet the rule requires: where
/// that feature is absent, no list of the kind has entries to write.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// in a [registration round](crate::core::registry::builder#the-two-registration-rounds)
/// that does not accept it (from a finalizer, for instance).
pub(super) fn declare_kind(
    b: &mut FeatureBuilder<'_>,
    kind: &Arc<XnoteKind>,
    list: Arc<ListRule>,
) -> Result<(), BuildError> {
    declare_backend(b)?;
    b.lowering_rule(list_html_rule(kind, list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rules_are_the_defaults_on_the_html_backend_only() {
        let rule = html_rule();
        assert_eq!(rule.name, HTML_RULE);
        assert_eq!(rule.element, XNOTE);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
        assert_eq!(list_rule_name("footnote").as_str(), "flm.xnotes.footnote.list.lower.html");
    }

    #[test]
    fn the_stylesheet_names_itself_and_is_one_layer() {
        let style = stylesheet();
        assert_eq!(style.id.as_str(), "flm.xnotes.style");
        let [DefContent::Inline(css)] = style.content.as_slice() else { panic!("one inline part") };
        assert_eq!(css.lines().next(), Some("/* flm.xnotes.style */"));
        assert_eq!(css.matches("@layer flm {").count(), 1);
    }
}
