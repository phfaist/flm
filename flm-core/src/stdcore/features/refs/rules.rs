//! The two lowering rules that the [`refs`](super) feature registers as the
//! default on every backend: [`REF_RULE`] lowers a `\ref` to links around
//! what its targets show, and [`HYPERREF_RULE`] lowers a `\hyperref` to a
//! link around the author's own text. A lowering rule
//! ([`LoweringRule`]) is written for one element and stages (builds into the
//! lowered tree; see [`Lowerer`]) the element's presentation for a backend.
//! On the `latex-recomposed` backend, the rules of [`super::latex`] are the
//! default instead.
//!
//! The module also holds [`lower_reference`], through which a command of
//! another feature shows a reference exactly as a `\ref` does.
//!
//! # What a `\ref` shows
//!
//! A `\ref` may point at several targets, so the rule reads every target the
//! node wrote ([`ref_targets`](super::ref_targets)) and lowers them through
//! [`lower_reference`].
//!
//! For each target, the rule shows the first of three things that exists:
//!
//! - the **number** the document gave the target, where the numbering stage
//!   recorded one for it, written from the counter table `flm.counters`; the
//!   rule reads that table optionally, through a peer declaration
//!   ([`PeerName`](crate::core::registry::PeerName)), so that the same rule
//!   works in a registry without the numbering feature;
//! - else the target's **content**: the synthesized value that the anchors
//!   stage copied out of the document (a heading's title, for instance), or
//!   the one that the target's element built (such as the bullet beside a
//!   list's item);
//! - else the **label** the reference was written with, as written: the
//!   target has nothing to show, which is reported as the Error
//!   `flm.refs.nothing-to-show` ([`NothingToShow`](super::NothingToShow)).
//!
//! Each of the three is placed inside a
//! [`flm.pres.link`](crate::stdcore::backends::pres::LINK) to the target,
//! whose kind is [`LINK_KIND`]. A target that nothing resolves, and a target
//! that is not spelled `type:label` at all, gets no link: its own text is
//! placed inside a [`flm.pres.span`](crate::stdcore::backends::pres::SPAN)
//! styled as an unresolved reference. The resolve stage ([`RESOLVE`](super::RESOLVE)) or the
//! label scan ([`SCAN`](super::SCAN)) has already reported it.
//!
//! # Several targets
//!
//! A reference is written as one list of items, joined by the texts of the
//! feature's configuration ([`RefsConfig`](super::RefsConfig)): by default,
//! two items read `Eq. (1) and Thm. 1.1`, and three or more read `Eq. (1),
//! Thm. 1.1, and A note`.
//!
//! The first items are the numbers of the targets the document numbered, one
//! item per [counter style](crate::stdcore::features::numbering::CounterStyleDecl)
//! (how the numbering feature shows a counter's value; see [counters and
//! counter styles](crate::guide::concepts_overview#counters-and-counter-styles)),
//! in the order the styles are first reached:
//! [`Counters::format_many`] groups the targets by the counter style they are
//! shown in and passes each style's numbers to that style's number writer,
//! which writes them as one text with the style's own joining texts
//! (`Eqs. (1–3), (7) and (9)`). The default number writer also groups a
//! style's numbers by the counters they are numbered within, and collapses
//! consecutive numbers of a group into a range. Each number links to its own
//! target, and the word before the numbers links to the first target where
//! the counter style asks for it. With exactly one target,
//! [`Counters::format`] writes the number, and the whole text is one link.
//!
//! Then come the other targets (one the document did not number, one that
//! nothing resolves, one that is not a label), one item each, in the order
//! the document wrote them, each shown as a single target is shown. The
//! texts that join the items link to nothing.
//!
//! # The display options
//!
//! The options a `\ref` carries ([`RefDisplayOptions`]) choose the wording of
//! the word a counter style places before a number: `noprefix` leaves the
//! word out, the key `S` asks for the capitalized wording (the variant
//! [`CAPITAL_VARIANT`](super::CAPITAL_VARIANT)), and any other key asks for
//! the variant of its own name. The options apply to every target of the
//! reference alike.
//!
//! `S` is a preference: a counter style that declares the variant `capital`
//! writes it, and a style that does not writes its usual wording, with
//! nothing reported. A variant written as the key itself (`capital` or
//! `abbreviated`) is a requirement: where the counter style of a numbered
//! target does not declare it, the reference is reported as the Error
//! `flm.refs.undeclared-variant` ([`UndeclaredVariant`](super::UndeclaredVariant)),
//! once per style that lacks it, and that style writes its usual wording. A
//! target shown by its content or its label has no counter style, and nothing
//! is checked for it.
//!
//! # `\hyperref`
//!
//! The `\hyperref` rule stages a link or an unresolved span in the same way,
//! with one difference: their content is the text the author wrote in the
//! braces, lowered as ordinary inline content. What the target has to show
//! takes no part, so the rule reads no synthesized value, no number and
//! nothing of the counter table. A `\hyperref` whose target is not spelled
//! `type:label` (an absent or unreadable target included) still shows the
//! author's text, without a link and styled as unresolved.
//!
//! # A reference site missing from the resolutions table
//!
//! A reference site that the table `flm.refs.resolutions` does not hold at
//! all differs from a target that nothing resolves: it means that the rule is
//! lowering content that the resolve stage ([`RESOLVE`](super::RESOLVE)) never saw, such as
//! a synthesized value that the code lowering it did not bring up to date
//! through value catch-up. The rule reports it at
//! the node as `flm.run.missing-entry` ([`MissingEntry`]) and then fails.

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef, NodeTree};
use techy::error::Diagnostic;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::model::aspects::{PointId, RefDisplayOptions};
use crate::model::element::element_of;
use crate::model::names::{CapName, ElementName};
use crate::core::run::MissingEntry;
use crate::lang::Flm;
use crate::stdcore::backends::lowering::lower_arg_named_or_nothing;
use crate::stdcore::backends::pres::{self, LinkPayload, SpanPayload, SpanStyle};
use crate::stdcore::features::numbering::{COUNTERS, Counters, FormatOptions, NumberSegment};

use super::elements::{
    CAPITAL_KEY, CAPITAL_VARIANT, HYPERREF, REF, RefTargetText, TEXT_ARG, hyperref_target,
    ref_display_options, ref_targets,
};
use super::list::{ListTexts, join_items};
use super::stages::{NothingToShow, UndeclaredVariant};
use super::tables::{REF_RESOLUTIONS, Resolution, SiteId};

/// The name of the `\ref` lowering rule, `flm.ref.lower.default`, whose
/// implementation is [`RefRule`]: the default for `flm.ref` on every backend
/// except `latex-recomposed`.
pub const REF_RULE: &str = "flm.ref.lower.default";

/// The name of the `\hyperref` lowering rule, `flm.hyperref.lower.default`,
/// whose implementation is [`HyperrefRule`]: the default for `flm.hyperref`
/// on every backend except `latex-recomposed`.
pub const HYPERREF_RULE: &str = "flm.hyperref.lower.default";

/// The kind token written on the link that a `\ref` or a `\hyperref`
/// produces, `ref`: it tells a backend, or a stylesheet, that the link comes
/// from a reference (see the [payload
/// conventions](crate::stdcore::backends::pres::payload)). The HTML backend
/// writes it as `data-flm-kind="ref"`.
pub const LINK_KIND: &str = "ref";

/// Stages `body` inside a
/// [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) styled as an
/// unresolved reference: what both rules produce where nothing resolved the
/// target.
///
/// # Errors
///
/// Returns [`LowerError`] when the node could not be built, or when the
/// calling rule did not declare that it produces `flm.pres.span`.
fn unresolved_node(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    body: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let payload =
        SpanPayload { style: Some(SpanStyle::UnresolvedReference), ..SpanPayload::default() };
    lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)
}

/// Stages `body` inside a
/// [`flm.pres.link`](crate::stdcore::backends::pres::LINK) to `href` of the
/// kind [`LINK_KIND`]: what both rules produce for a resolved target.
///
/// # Errors
///
/// Returns [`LowerError`] when the node could not be built, or when the
/// calling rule did not declare that it produces `flm.pres.link`.
fn link_node(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    href: String,
    body: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let payload = LinkPayload {
        href,
        kind: LINK_KIND.into(),
        doc_role: None,
        label: None,
        shows_target: false,
    };
    lw.pres_with(cx, &ElementName::from(pres::LINK), payload, body)
}

/// How the reference site `index` of `node` resolved, read from
/// `flm.refs.resolutions` at the node's own address.
///
/// `None` is not a site that nothing resolved (that is
/// [`Resolution::Unresolved`]) but a site the table does not hold at all,
/// which means that the rule is lowering content the resolve stage
/// ([`RESOLVE`](super::RESOLVE)) never saw: a synthesized value that the code lowering it
/// did not bring up to date through value catch-up. It is
/// reported at the node as `flm.run.missing-entry`, giving `element`, and the
/// caller fails.
fn resolution_of(
    node: &NodeRef<'_, Flm>,
    lw: &mut dyn Lowerer,
    element: &str,
    index: u32,
) -> Result<Option<Resolution>, LowerError> {
    let site = SiteId { addr: lw.addr(), index };
    // The resolution is cloned out of the table (a value tree is one shared
    // pointer): splicing needs the `Lowerer` mutably.
    let resolution = lw.get(REF_RESOLUTIONS)?.and_then(|table| table.site(site)).cloned();
    if resolution.is_none() {
        let table = CapName::from(REF_RESOLUTIONS.name());
        let producer = lw
            .producer_of(&table)
            .map(|stage| stage.to_string())
            .unwrap_or_else(|| "nobody".to_string());
        lw.diagnose(Diagnostic::error(
            MissingEntry { table: table.to_string(), producer, element: element.to_string() },
            node.span().clone(),
        ));
    }
    Ok(resolution)
}

/// What one target of a reference has to show, once it has been resolved.
enum Shown {
    /// The target was found: the link target, the reference point it
    /// designates in this document (absent for a target of another document),
    /// the content it shows where it has no number, and the text the
    /// reference wrote for it.
    Resolved {
        href: String,
        point: Option<PointId>,
        content: Option<Arc<NodeTree<Flm>>>,
        text: String,
    },
    /// Nothing resolved the target, or its text is not a label: this text is
    /// shown, without a link and marked unresolved.
    Unresolved { text: String },
}

/// The wording variant that `display` asks for, if any: its name, and whether
/// the request is a requirement. The key `S` asks for the variant `capital`
/// as a preference, and a counter style that lacks it writes its usual
/// wording; any other key asks for the variant of its own name as a
/// requirement, and a counter style that lacks it is reported at the
/// reference.
fn requested_variant(display: &RefDisplayOptions) -> Option<(&str, bool)> {
    display.variant.as_deref().map(|variant| match variant {
        CAPITAL_KEY => (CAPITAL_VARIANT, false),
        other => (other, true),
    })
}

/// The formatting options with which a counter style writes a number at a
/// reference site: the display options translated into the options of the
/// numbering feature.
///
/// `noprefix` leaves out the word before the number, and the variant asked
/// for is [`requested_variant`]. The delimiters around a number are always
/// written: no option of a reference asks for a number without them.
fn format_options(display: &RefDisplayOptions) -> FormatOptions {
    let mut options = FormatOptions::new();
    if !display.with_prefix {
        options = options.without_word();
    }
    if let Some((variant, _)) = requested_variant(display) {
        options = options.variant(variant);
    }
    options
}

/// The names of the counter styles of the points `numbered` that do not
/// declare the wording variant `variant`: each once, in the order of the
/// first point that uses it.
fn styles_lacking(counters: &Counters, numbered: &[PointId], variant: &str) -> Vec<String> {
    let mut lacking: Vec<String> = Vec::new();
    for point in numbered {
        let Some(record) = counters.record(*point) else { continue };
        if record.style.declares_variant(variant) {
            continue;
        }
        let name = record.style.name().as_str();
        if !lacking.iter().any(|seen| seen == name) {
            lacking.push(name.to_string());
        }
    }
    lacking
}

/// Lowers a reference to `targets`, with the display options `display`, into
/// the items it shows: a command of another feature calls this function to
/// show a reference exactly as a `\ref` does, instead of writing that
/// behavior again.
///
/// Returns the list of items, in order, each a sequence of staged nodes
/// linked to its own target: first one item per counter style for the
/// targets the document numbered, then one item per other target. The module
/// documentation says what each item shows. The `\ref` rule joins the items
/// with the texts of [`RefsConfig`](super::RefsConfig), even a single item,
/// which receives `one_pre` and `one_post`. A command of another feature
/// writes the items as it needs. Two do so: the mathematics feature's
/// `\eqref`, whose one target shows what `\ref[noprefix]` would show, writes
/// its one item without `one_pre` and `one_post` (both empty by default),
/// because that feature cannot read this feature's joining texts; and the
/// theorems feature's back-reference from a proof to its theorem.
///
/// `node` is the node being lowered, whose address identifies the reference
/// sites the resolve stage ([`RESOLVE`](super::RESOLVE)) wrote for it;
/// `targets` are the targets of that
/// node, in the order they were written, as [`ref_targets`](super::ref_targets)
/// reads them; `display` holds the display options. A wording variant written
/// as a key that the counter style of a numbered target does not declare is
/// reported at `node` ([`UndeclaredVariant`](super::UndeclaredVariant)); the
/// key `S` is a preference and is never reported.
///
/// The calling rule must declare that it produces
/// [`flm.pres.link`](crate::stdcore::backends::pres::LINK) and
/// [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) and that it reads
/// `flm.refs.resolutions` ([`REF_RESOLUTIONS`]). It should also declare an
/// optional read of [`COUNTERS`], with a peer declaration of that table
/// ([`FeatureBuilder::peer_table`](crate::core::registry::FeatureBuilder::peer_table)),
/// so that a target the document numbered shows its number.
///
/// The calling *element* must declare, through
/// [`Referencing`](crate::model::aspects::Referencing), one reference site
/// per target of `targets` that has a label, in the order those targets are
/// written: the resolution of a target is read by its place among the
/// labeled targets. A target without a label uses no site and is shown
/// unresolved, as the text it was written with. If the element declares its
/// sites in any other way, the rule reports `flm.run.missing-entry` at the
/// node and fails, which looks like a fault of the pipeline although the
/// mistake is in the calling feature.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be built, when the calling
/// rule did not declare what this function stages or reads, when a table
/// could not be read, or when the reference site of a target is missing from
/// `flm.refs.resolutions` (a fault of an earlier step, reported at the node
/// before the error is returned).
pub fn lower_reference(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    targets: &[RefTargetText],
    display: &RefDisplayOptions,
) -> Result<Vec<Vec<BuildId>>, LowerError> {
    let element = element_of(node).name.to_string();
    // A target that is no label declares no reference site, so the site of a
    // target is its place among the labeled targets — the same count the
    // `Referencing` implementation makes when it declares them.
    let mut site = 0u32;
    let mut shown: Vec<Shown> = Vec::with_capacity(targets.len());
    for target in targets {
        if target.label.is_none() {
            shown.push(Shown::Unresolved { text: target.text.clone() });
            continue;
        }
        let index = site;
        site += 1;
        match resolution_of(node, lw, &element, index)? {
            Some(Resolution::Resolved { point, href, display }) => shown.push(Shown::Resolved {
                href,
                point,
                content: display.content,
                text: target.text.clone(),
            }),
            Some(Resolution::Unresolved { target }) => {
                shown.push(Shown::Unresolved { text: target.to_string() })
            }
            None => return Err(LowerError::structure("the reference has no resolution entry")),
        }
    }

    // The points this document numbered, and the numbers written for them.
    // Both are read in one borrow of the counter table, and what comes out of
    // it is owned, because staging content needs the `Lowerer` mutably.
    let points: Vec<PointId> = shown
        .iter()
        .filter_map(|target| match target {
            Shown::Resolved { point, .. } => *point,
            Shown::Unresolved { .. } => None,
        })
        .collect();
    // A variant named outright that the style of a numbered target does not
    // declare is reported here, once per style; the style writes its usual
    // wording all the same. The key `S` is a preference, and is not checked.
    let variant = requested_variant(display)
        .and_then(|(variant, demanded)| demanded.then_some(variant));
    let (numbered, groups, lacking) = match lw.get(COUNTERS)? {
        None => (Vec::new(), Vec::new(), Vec::new()),
        Some(counters) => {
            let (numbered, groups) = written_numbers(counters, &points, targets.len(), display)?;
            let lacking = match variant {
                Some(variant) => styles_lacking(counters, &numbered, variant),
                None => Vec::new(),
            };
            (numbered, groups, lacking)
        }
    };
    if let Some(variant) = variant {
        for style in lacking {
            lw.error(UndeclaredVariant { variant: variant.to_string(), style }, node.span().clone());
        }
    }

    let hrefs: BTreeMap<PointId, String> = shown
        .iter()
        .filter_map(|target| match target {
            Shown::Resolved { point: Some(point), href, .. } => Some((*point, href.clone())),
            _ => None,
        })
        .collect();
    let mut items: Vec<Vec<BuildId>> = Vec::with_capacity(groups.len() + shown.len());
    for group in &groups {
        let mut item = Vec::new();
        for segment in group {
            let body = lw.splice(cx, segment.content())?;
            match segment.link().and_then(|point| hrefs.get(&point)) {
                Some(href) => item.push(link_node(cx, lw, href.clone(), body)?),
                None => item.extend(body),
            }
        }
        items.push(item);
    }
    for target in &shown {
        match target {
            Shown::Resolved { point: Some(point), .. } if numbered.contains(point) => continue,
            _ => {}
        }
        let node = match target {
            Shown::Resolved { href, content, text, .. } => {
                let body = match content {
                    Some(value) => lw.splice(cx, value)?,
                    None => {
                        let span = node.span().clone();
                        lw.error(NothingToShow { label: text.clone() }, span);
                        vec![lw.text(cx, text)?]
                    }
                };
                link_node(cx, lw, href.clone(), body)?
            }
            Shown::Unresolved { text } => {
                let body = vec![lw.text(cx, text)?];
                unresolved_node(cx, lw, body)?
            }
        };
        items.push(vec![node]);
    }
    Ok(items)
}

/// The points of `points` that the document numbered, and their numbers: one
/// group of segments per counter style.
///
/// `count` is the number of targets of the reference: with exactly one,
/// [`Counters::format`] writes the number as one group whose whole text is
/// one link; with more, [`Counters::format_many`] groups the points by
/// counter style and lets each style's number writer write them.
///
/// # Errors
///
/// Returns [`LowerError`] when a number could not be written, which means
/// that the counter record and its counter style do not fit each other: a
/// fault of an earlier step, since the numbering stage refuses a record its
/// counter style cannot write.
fn written_numbers(
    counters: &Counters,
    points: &[PointId],
    count: usize,
    display: &RefDisplayOptions,
) -> Result<(Vec<PointId>, Vec<Vec<NumberSegment>>), LowerError> {
    let numbered: Vec<PointId> =
        points.iter().copied().filter(|point| counters.record(*point).is_some()).collect();
    if numbered.is_empty() {
        return Ok((numbered, Vec::new()));
    }
    let options = format_options(display);
    let written = if count == 1 {
        counters.format(numbered[0], &options).unwrap_or(Ok(Vec::new())).map(|one| vec![one])
    } else {
        counters.format_many(&numbered, &options).map(|many| many.groups)
    };
    let groups = written.map_err(|error| LowerError::structure(error.to_string()))?;
    Ok((numbered, groups))
}

/// The implementation of [`REF_RULE`]: it stages what each target of the
/// reference shows (its number, its content or its label), linked to that
/// target, as one list joined by the texts of
/// [`RefsConfig`](super::RefsConfig). See the module documentation.
///
/// The feature registers it with the joining texts of its configuration,
/// compiled when the registry is built.
#[derive(Debug)]
pub struct RefRule {
    list: Arc<ListTexts>,
}

impl Lower for RefRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let targets = ref_targets(&node).unwrap_or_default();
        let display = ref_display_options(&node);
        let items = lower_reference(cx, lw, &node, &targets, &display)?;
        Ok(Lowering::Emit(join_items(cx, lw, &self.list, items)?))
    }
}

/// The implementation of [`HYPERREF_RULE`]: it stages the text the author
/// wrote inside a [`flm.pres.link`](crate::stdcore::backends::pres::LINK) to
/// the target, or inside a
/// [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) styled as an
/// unresolved reference where nothing resolved the target. See the module
/// documentation.
#[derive(Debug, Default)]
pub struct HyperrefRule;

impl Lower for HyperrefRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        // The author's text is what this shows, resolved or not, so it is
        // lowered once, before anything is looked up. A text a parse recovery
        // left absent lowers to nothing and is not reported a second time.
        let body = lower_arg_named_or_nothing(cx, lw, &node, TEXT_ARG)?;
        let target = hyperref_target(&node)
            .unwrap_or(RefTargetText { text: String::new(), label: None });
        if target.label.is_none() {
            return Ok(Lowering::Emit(vec![unresolved_node(cx, lw, body)?]));
        }
        let node = match resolution_of(&node, lw, HYPERREF, 0)? {
            // The target's own display takes no part here.
            Some(Resolution::Resolved { href, .. }) => link_node(cx, lw, href, body)?,
            Some(Resolution::Unresolved { .. }) => unresolved_node(cx, lw, body)?,
            None => return Err(LowerError::structure("the reference has no resolution entry")),
        };
        Ok(Lowering::Emit(vec![node]))
    }
}

/// The feature's two lowering rules, ready to be registered: the `\ref` rule,
/// which joins its items with the texts that `list` holds once compiled, and
/// the `\hyperref` rule, each the default on every backend for its own
/// element.
///
/// The `\ref` rule reads the counter table `flm.counters` optionally, as a
/// table of another feature ([`LoweringDecl::reads_opt`]): where the
/// numbering feature is registered, the stage that writes the table is
/// scheduled; where it is not, the rule shows a target's content, or its label with the Error
/// `flm.refs.nothing-to-show` where it has no content.
pub(super) fn rules(list: &Arc<ListTexts>) -> Vec<LoweringRule> {
    let decl = || {
        LoweringDecl::new().reads(REF_RESOLUTIONS).produces(pres::LINK).produces(pres::SPAN)
    };
    vec![
        LoweringRule::new(
            REF_RULE,
            REF,
            decl().reads_opt(COUNTERS),
            Arc::new(RefRule { list: Arc::clone(list) }),
            DefaultMark::Everywhere,
        )
        .docs(
            "a `\\ref` lowers to one `flm.pres.link` per resolved target, around the target's \
             number, its content or its label, and to a `flm.pres.span` styled as unresolved \
             around the text of a target that nothing resolves",
        ),
        LoweringRule::new(
            HYPERREF_RULE,
            HYPERREF,
            decl(),
            Arc::new(HyperrefRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "a `\\hyperref` lowers to a `flm.pres.link` around the text the author wrote when \
             its target resolves, and to a `flm.pres.span` styled as unresolved around the \
             same text when it does not",
        ),
    ]
}
