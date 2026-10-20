//! The general lowering rules of the `lists` feature: [`ListRule`], the rule
//! of a list on every backend except `latex-recomposed`, and [`ItemRule`],
//! the rule of a misplaced `\item` on every backend.
//!
//! - [`ListRule`] (the rule [`RULE`](super::RULE)) lowers a list to one
//!   [`flm.pres.list`](crate::stdcore::backends::pres::LIST) node holding, per
//!   item, a `label` content slot and an `item` content slot.
//! - [`ItemRule`] (the rule [`ITEM_RULE`](super::ITEM_RULE)) lowers an
//!   `\item` that begins no item to nothing, and reports it.
//!
//! The helpers that report the mistakes of a list ([`report_reading`],
//! [`own_label`], [`own_ref`], [`report_no_numbering`]) are shared with the
//! recomposition rule of the `latex-recomposed` backend
//! ([`latex`](super::latex)), so that a list is reported alike on every
//! backend.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef, NodeTree};
use techy::error::Diagnostic;

use crate::core::lowering::{Lower, LowerCx, LowerError, Lowerer, Lowering};
use crate::core::run::MissingEntry;
use crate::lang::{Flm, FlmSpan};
use crate::model::aspects::PointId;
use crate::model::element::element_value;
use crate::model::names::{CapName, ElementName, Name};
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::backends::pres::{self, ListPayload, ITEM_SLOT, LABEL_SLOT};
use crate::stdcore::features::numbering::{
    CounterStyle, Counters, FormatOptions, NumberSegment, NumberTemplate, COUNTERS,
};
use crate::stdcore::features::refs::ANCHORS;

use super::diagnostics::{
    BadLabel, BadLabelForm, ContentBeforeItem, ItemOutsideList, LabelNamesNumber, MissingTag,
    MissingValue, NoNumbering, RefIgnored, UnknownKey,
};
use super::items::{list_items, ListBody};
use super::labels::{CompiledLabel, Labels};
use super::syntax::{item_options, list_options, ItemTag, OptionProblem};
use crate::lang::copy::WrittenPiece;
use super::{ListElement, ITEM, ITEM_COMMAND, LABEL_KEY, REF_KEY};

/// What the items of one list show beside them where they carry no tag.
enum ListLabel {
    /// The number recorded for each item, written through the list's label
    /// template: its own where it writes one that compiles (the text), else
    /// the one its list kind declares for its depth. The numbering feature's
    /// stage [`NUMBER`](crate::stdcore::features::numbering::NUMBER), which
    /// records each reference point's counter value in the counter table,
    /// records the numbers.
    Numbers(Option<String>),
    /// One fixed label, the same beside every item.
    Fixed(Arc<NodeTree<Flm>>),
    /// Nothing, and nothing more to report: the list's numbers cannot be
    /// shown, which has been reported once for the list.
    Nothing,
    /// Nothing, because the list writes no label that can be used: an item
    /// without a tag is an error, reported by [`report_reading`] or as the
    /// problem of the list's own label.
    Missing,
}

/// What one item shows beside it.
enum ItemLabel<'a, 't> {
    /// Its tag.
    Tag(&'a ItemTag<'t>),
    /// Content ready to be spliced: a fixed label.
    Tree(Arc<NodeTree<Flm>>),
    /// Its number, as the counter table writes it.
    Number(Vec<NumberSegment>),
    /// Nothing.
    Nothing,
}

/// Reports the mistakes in how the list `node` is written, its body read as
/// `body`: what its optional argument and its items' optional arguments hold
/// that their readers cannot read ([`UnknownKey`], [`MissingValue`],
/// [`BadLabelForm`]); a `ref=` on a list whose list kind is not numbered (the
/// Note [`RefIgnored`]); content written before its first `\item`
/// ([`ContentBeforeItem`]); and each item carrying no tag in a list that
/// writes no label, because its list kind declares no template for the depth
/// and its optional argument gives none that can be read ([`MissingTag`]).
///
/// Every rule that lowers a list calls it, the general rule and the
/// recomposition rule alike, so a list's mistakes are reported whichever
/// backend renders it. The problems of the label beside an item (a template
/// that does not compile, a number with nothing to number it) are reported
/// by [`own_label`], [`own_ref`] and [`report_no_numbering`], which both
/// rules call too.
pub(super) fn report_reading(
    node: &NodeRef<'_, Flm>,
    value: &ListElement,
    body: &ListBody<'_>,
    lw: &mut dyn Lowerer,
) {
    let environment = value.name.as_str();
    let options = list_options(node);
    report_problems(&options.problems, environment, node.span(), lw);
    // Accepted and set aside, so that a list switched between a numbered and
    // an unnumbered kind keeps its bracket.
    // See decision rationale at [§flm-dd-dr:std-item-labels].
    if options.ref_label.is_some() && !value.kind.numbered {
        lw.diagnose(Diagnostic::note(
            RefIgnored { environment: environment.to_string() },
            node.span().clone(),
        ));
    }
    if let Some(first) = body.before.first() {
        lw.diagnose(Diagnostic::error(
            ContentBeforeItem { environment: environment.to_string() },
            first.span().clone(),
        ));
    }
    let writes_no_label = body.kind_label.is_none() && body.own_label.is_none();
    for item in &body.items {
        report_problems(&item_options(&item.node).problems, ITEM_COMMAND, item.node.span(), lw);
        if writes_no_label && item.tag.is_none() {
            lw.diagnose(Diagnostic::error(
                MissingTag { environment: environment.to_string() },
                item.node.span().clone(),
            ));
        }
    }
}

/// Reports, at `span`, what an optional argument held that its reader could
/// not read.
fn report_problems(problems: &[OptionProblem], construct: &str, span: &FlmSpan, lw: &mut dyn Lowerer) {
    for problem in problems {
        let construct = construct.to_string();
        lw.diagnose(match problem.clone() {
            OptionProblem::UnknownKey(key) => {
                Diagnostic::error(UnknownKey { construct, key }, span.clone())
            }
            OptionProblem::MissingValue(key) => {
                Diagnostic::error(MissingValue { construct, key }, span.clone())
            }
            OptionProblem::BadLabelForm { written, first, second } => {
                Diagnostic::error(BadLabelForm { environment: construct, written, first, second }, span.clone())
            }
        });
    }
}

/// The lowering rule [`RULE`](super::RULE) of a list, the default on every
/// backend except `latex-recomposed`. It stages (builds into the lowered tree;
/// see [`Lowerer`]) one
/// [`flm.pres.list`](crate::stdcore::backends::pres::LIST) node whose kind
/// tokens are the list kind's structural token, the list environment's name
/// and [`LIST_LABELED`](crate::stdcore::backends::pres::LIST_LABELED), and
/// which holds one entry per item: the item's label in the `label` content
/// slot, the item's content lowered through the rules in the `item` content
/// slot, and the item's anchor on the payload
/// ([`ListPayload`]). The [module documentation of the
/// feature](super#how-a-list-is-lowered) describes the result in full. The
/// rule reports every mistake in how the list is written and every problem of
/// its labels (see the feature's [diagnostics](super#diagnostics)).
///
/// The feature creates the rule and gives it the list kinds' compiled label
/// templates.
#[derive(Debug)]
pub struct ListRule {
    labels: Arc<Labels>,
}

impl ListRule {
    /// The rule over `labels`, the compiled templates of the feature's list
    /// kinds.
    pub(super) fn new(labels: Arc<Labels>) -> ListRule {
        ListRule { labels }
    }

    /// What the items of the list `node` show where they carry no tag. The
    /// decision reports the problems it finds: a template the rule compiles
    /// that does not compile, an unnumbered list's template that writes a
    /// number, a numbered list with nothing to number it.
    fn list_label(
        &self,
        node: &NodeRef<'_, Flm>,
        value: &ListElement,
        body: &ListBody<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<ListLabel, LowerError> {
        // Under the numbering feature, the numbering stage compiles the list's
        // own reference template — reporting it once where it does not
        // compile — and records every item's number in the style it is
        // written in; the rule writes each recorded number through the label
        // template. The list's own label template is compiled and reported
        // here, once per list, whatever its items are, as the recomposition
        // rule does; the style it is written in is derived from it once
        // ([`Self::label_style`]).
        if value.kind.numbered && lw.get(COUNTERS)?.is_some() {
            let own = own_label(&self.labels, node, value, body, true, lw);
            return Ok(ListLabel::Numbers(own.map(|own| own.template().text().to_string())));
        }
        let own = own_label(&self.labels, node, value, body, false, lw);
        own_ref(&self.labels, node, value, body, false, lw);
        report_no_numbering(&self.labels, node, value, body, own.as_ref(), lw)?;
        let Some(label) = own.as_ref().or_else(|| self.labels.of_kind(&value.name, body.depth))
        else {
            return Ok(ListLabel::Missing);
        };
        match label.fixed().map_err(LowerError::other)? {
            Some(tree) => Ok(ListLabel::Fixed(tree)),
            // A numbered list with no numbering feature, whose label writes a
            // number: there is none to write, and no second count stands in
            // for it; reported above.
            None => Ok(ListLabel::Nothing),
        }
    }

    /// The counter style in which the label beside an item of a list is
    /// written: `recorded`, the counter style the item's counter value carries
    /// (whose number format is the list's reference template), with its
    /// number format replaced by the list's label template. That template is
    /// `own` where the list writes one that compiles ([`own_label`] compiled
    /// and reported it), else the one its list kind declares for its depth.
    /// `None` only where nothing can be compiled, before the registry's
    /// templates were compiled, which cannot happen with a built registry.
    // The text is compiled a second time here, as a number format, since
    // the compiled label holds a template, not the compiled number format a
    // counter style needs; once per list.
    // See decision rationale at [§flm-dd-dr:std-item-labels].
    fn label_style(
        &self,
        body: &ListBody<'_>,
        recorded: &CounterStyle,
        own: Option<&str>,
        lw: &dyn Lowerer,
    ) -> Option<CounterStyle> {
        let language = self.labels.language()?;
        let derive = |text: &str| {
            recorded.with_number_format(&NumberTemplate::formats(text), lw.lang(), language).ok()
        };
        own.and_then(derive).or_else(|| derive(body.kind_label.as_deref()?))
    }

    /// The number of every untagged item of the list `body`, written through
    /// its label template ([`label_style`](Self::label_style)), without the
    /// word and the delimiters, which belong to a reference to the item and
    /// not to its label; `None` for an item with no number to write. The
    /// result also says whether an item's reference point is missing from the
    /// counter table altogether.
    ///
    /// # Errors
    ///
    /// [`LowerError`] when an item is not a node of the tree being lowered,
    /// or a recorded number cannot be written in its style.
    fn numbers(
        &self,
        body: &ListBody<'_>,
        counters: &Counters,
        own: Option<&str>,
        lw: &dyn Lowerer,
    ) -> Result<LabelNumbers, LowerError> {
        let mut out = LabelNumbers { numbers: vec![None; body.items.len()], missing: false };
        let options = FormatOptions::new().without_word().without_delimiters();
        // Every item of one list is recorded in one style; the label style is
        // derived from it once, and again only for a record carrying another.
        let mut derived: Option<(Arc<CounterStyle>, Option<CounterStyle>)> = None;
        for (index, item) in body.items.iter().enumerate() {
            if item.tag.is_some() {
                continue;
            }
            let point = PointId { addr: lw.addr_of(&item.node)?, index: 0 };
            let record = match counters.lookup(point) {
                // A point the stage never saw — an item of a list inside a
                // synthesized value — has no number: the list is reported
                // once, and the item shows no label.
                None => {
                    out.missing = true;
                    continue;
                }
                // A point the stage saw and did not number, which the stage
                // reported: the item shows no label.
                Some(None) => continue,
                Some(Some(record)) => record,
            };
            if !derived.as_ref().is_some_and(|(of, _)| Arc::ptr_eq(of, &record.style)) {
                let style = self.label_style(body, &record.style, own, lw);
                derived = Some((Arc::clone(&record.style), style));
            }
            let Some((_, Some(style))) = &derived else { continue };
            out.numbers[index] = counters
                .format_in_style(point, style, &options)
                .transpose()
                .map_err(|error| LowerError::structure(error.to_string()))?;
        }
        Ok(out)
    }

    /// Stages one item's label.
    fn stage_label(
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
        label: ItemLabel<'_, '_>,
    ) -> Result<Vec<BuildId>, LowerError> {
        match label {
            ItemLabel::Nothing => Ok(Vec::new()),
            ItemLabel::Tree(tree) => lw.splice(cx, &tree),
            ItemLabel::Number(segments) => {
                let mut staged = Vec::new();
                for segment in &segments {
                    staged.extend(lw.splice(cx, segment.content())?);
                }
                Ok(staged)
            }
            ItemLabel::Tag(tag) => stage_tag(cx, lw, tag),
        }
    }
}

/// What [`ListRule::numbers`] returns: the number of each item, and whether
/// an item's reference point is missing from the counter table.
struct LabelNumbers {
    numbers: Vec<Option<Vec<NumberSegment>>>,
    missing: bool,
}

/// Whether the numbering stage compiles `text` for the list `value`, `body`,
/// and reports it where it does not compile: true when `counted` (a numbered
/// list in a registry with the numbering feature) and `text` is the list's own
/// reference template, which its items carry to the stage.
fn stage_compiles(value: &ListElement, body: &ListBody<'_>, counted: bool, text: &str) -> bool {
    counted && value.kind.numbered && body.own_ref.as_deref() == Some(text)
}

/// The list `node`'s own label template, compiled, where the list writes one
/// that can be used; `None` where it writes none, or one that cannot be used,
/// which is reported: a template that writes a number on an unnumbered list
/// ([`LabelNamesNumber`]), and a template that does not compile
/// ([`BadLabel`]), unless the numbering stage compiles the same text and
/// reports it. `counted` says that the list is numbered in a registry with
/// the numbering feature, whose stage compiles the list's reference template;
/// a mistake is therefore reported once. The warnings raised when compiling
/// a template that can be used are reported under the same rule.
///
/// The template is compiled against the language the [`Lowerer`] gives the
/// rule, since nothing else compiles it where the numbering stage does not.
// See decision rationale at [§flm-dd-dr:rend-rule-lang].
pub(super) fn own_label(
    labels: &Labels,
    node: &NodeRef<'_, Flm>,
    value: &ListElement,
    body: &ListBody<'_>,
    counted: bool,
    lw: &mut dyn Lowerer,
) -> Option<CompiledLabel> {
    let own = body.own_label.as_ref()?;
    match labels.compile(&own.template, "the list's label", lw.lang()) {
        Err(detail) => {
            if !stage_compiles(value, body, counted, &own.template) {
                lw.diagnose(Diagnostic::error(
                    BadLabel {
                        environment: value.name.to_string(),
                        key: LABEL_KEY.to_string(),
                        template: own.template.clone(),
                        detail,
                    },
                    node.span().clone(),
                ));
            }
            None
        }
        Ok(label) if label.names_number() && !value.kind.numbered => {
            lw.diagnose(Diagnostic::error(
                LabelNamesNumber {
                    environment: value.name.to_string(),
                    template: own.template.clone(),
                },
                node.span().clone(),
            ));
            None
        }
        Ok(label) => {
            report_warnings(&label, stage_compiles(value, body, counted, &own.template), lw);
            Some(label)
        }
    }
}

/// Reports the warnings raised when compiling `label`, unless `reported`
/// says that the numbering stage compiles the same text and reports them.
fn report_warnings(label: &CompiledLabel, reported: bool, lw: &mut dyn Lowerer) {
    if !reported {
        for warning in label.template().diagnostics() {
            lw.diagnose(warning.clone());
        }
    }
}

/// The numbered list `node`'s own reference template (its `ref=`, else the
/// one its own label gives), compiled, where the list writes one that
/// compiles. `None` for an unnumbered list, whose `ref=` is not used
/// ([`RefIgnored`]), and where the list writes none or one that does not
/// compile. A template that does not compile is reported ([`BadLabel`]),
/// unless it is the text of the list's own label template, which
/// [`own_label`] reports, or the numbering stage compiles it (`counted`, as
/// for [`own_label`]); the same holds for the warnings raised when compiling
/// a template that compiles.
pub(super) fn own_ref(
    labels: &Labels,
    node: &NodeRef<'_, Flm>,
    value: &ListElement,
    body: &ListBody<'_>,
    counted: bool,
    lw: &mut dyn Lowerer,
) -> Option<CompiledLabel> {
    if !value.kind.numbered {
        return None;
    }
    let text = body.own_ref.as_deref()?;
    let label_text = body.own_label.as_ref().map(|own| own.template.as_str());
    let reported = label_text == Some(text) || stage_compiles(value, body, counted, text);
    match labels.compile(text, "the list's reference template", lw.lang()) {
        Ok(label) => {
            report_warnings(&label, reported, lw);
            Some(label)
        }
        Err(detail) => {
            if !reported {
                lw.diagnose(Diagnostic::error(
                    BadLabel {
                        environment: value.name.to_string(),
                        key: REF_KEY.to_string(),
                        template: text.to_string(),
                        detail,
                    },
                    node.span().clone(),
                ));
            }
            None
        }
    }
}

/// Reports [`NoNumbering`] for the list `node` where its items would show a
/// number and nothing numbers them: a numbered list, in a registry without
/// the numbering feature (the plan computes no counter table), whose label
/// writes a number, and which holds an item carrying no tag. The label is
/// `own` (the list's own template, where it writes one that can be used),
/// else its list kind's template for its depth. Reported once per list.
///
/// Every rule that lowers a list calls it, so that a document gets the same
/// report whichever backend renders it: the general rule, which then shows no
/// label beside those items, and the recomposition rule, which still writes
/// the list and leaves its numbers to LaTeX.
///
/// # Errors
///
/// [`LowerError`] when the rule did not declare that it reads the counter
/// table.
pub(super) fn report_no_numbering(
    labels: &Labels,
    node: &NodeRef<'_, Flm>,
    value: &ListElement,
    body: &ListBody<'_>,
    own: Option<&CompiledLabel>,
    lw: &mut dyn Lowerer,
) -> Result<(), LowerError> {
    if !value.kind.numbered || lw.get(COUNTERS)?.is_some() {
        return Ok(());
    }
    let shown = own.or_else(|| labels.of_kind(&value.name, body.depth));
    let untagged = body.items.iter().any(|item| item.tag.is_none());
    if untagged && shown.is_some_and(CompiledLabel::names_number) {
        lw.diagnose(Diagnostic::error(
            NoNumbering { environment: value.name.to_string() },
            node.span().clone(),
        ));
    }
    Ok(())
}

/// The kind tokens of a list of `value`'s list kind: first the structural
/// token of the list kind ([`LIST_NUMBERED`](pres::LIST_NUMBERED) for a
/// numbered list kind, [`LIST_UNNUMBERED`](pres::LIST_UNNUMBERED) for an
/// unnumbered list kind that declares label templates,
/// [`LIST_TERMS`](pres::LIST_TERMS) for a list kind that declares none, whose
/// items are labeled by their tags), then the environment's name, then
/// [`LIST_LABELED`](pres::LIST_LABELED), since the rule writes every item's
/// label itself.
fn kind_tokens(value: &ListElement) -> Vec<Name> {
    let structure = if value.kind.numbered {
        pres::LIST_NUMBERED
    } else if value.kind.labels.is_empty() {
        pres::LIST_TERMS
    } else {
        pres::LIST_UNNUMBERED
    };
    vec![Name::from(structure), value.name.clone(), Name::from(pres::LIST_LABELED)]
}

/// The anchor of each item of the list `node`, read from the references
/// feature's anchors table ([`ANCHORS`]): the identifier that the references
/// feature's stage
/// [`ANCHORS_STAGE`](crate::stdcore::features::refs::ANCHORS_STAGE), which
/// gives each reference point its anchor, gave the item's reference point;
/// `None` for an item whose anchor an earlier point had already taken. Without an anchors table (a registry
/// without the references feature, or a plan that computes no anchors), no
/// item has an anchor, and nothing is reported.
///
/// An item for which the table holds no entry at all has no anchor either:
/// its reference point is one the anchors stage never saw, an item of a list
/// inside a
/// [synthesized value](crate::guide::concepts_overview#synthesized-values)
/// (for example a label template that holds a list).
/// The list is then reported once ([`report_missing_entry`]) and still
/// written, since an item without an identifier shows nothing wrong.
///
/// # Errors
///
/// [`LowerError`] when an item is not a node of the tree being lowered, or
/// the rule did not declare that it reads the anchors table.
fn item_anchors(
    node: &NodeRef<'_, Flm>,
    body: &ListBody<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Vec<Option<String>>, LowerError> {
    let points = item_points(body, lw)?;
    let mut missing = false;
    let anchors = {
        let Some(anchors) = lw.get(ANCHORS)? else { return Ok(vec![None; body.items.len()]) };
        points
            .into_iter()
            .map(|point| match anchors.lookup(point) {
                None => {
                    missing = true;
                    None
                }
                Some(anchor) => anchor.map(String::from),
            })
            .collect::<Vec<_>>()
    };
    if missing {
        report_missing_entry(node, ANCHORS.name(), lw);
    }
    Ok(anchors)
}

/// The reference point of each item of `body`, in order.
///
/// # Errors
///
/// [`LowerError`] when an item is not a node of the tree being lowered.
fn item_points(body: &ListBody<'_>, lw: &mut dyn Lowerer) -> Result<Vec<PointId>, LowerError> {
    body.items
        .iter()
        .map(|item| lw.addr_of(&item.node).map(|addr| PointId { addr, index: 0 }))
        .collect()
}

/// Reports, at the list `node`, that the table `table` holds no entry at all
/// for the reference point of one or more of its items ([`MissingEntry`]),
/// giving the stage the plan scheduled to write the table: the list is
/// inside a synthesized value, which that stage never saw. The list's rule
/// calls it once per list and table, and still writes the list, with no
/// anchor or no label for those items.
fn report_missing_entry(node: &NodeRef<'_, Flm>, table: &str, lw: &mut dyn Lowerer) {
    let table = CapName::from(table);
    let producer =
        lw.producer_of(&table).map(|stage| stage.to_string()).unwrap_or_else(|| "nobody".to_string());
    lw.diagnose(Diagnostic::error(
        MissingEntry { table: table.to_string(), producer, element: ITEM.to_string() },
        node.span().clone(),
    ));
}

/// Stages an item's tag: each node of the document the tag is written as,
/// lowered through the rules, and each piece of text as a text node.
///
/// # Errors
///
/// [`LowerError`] from the lowering of a node or the building of a text node.
pub(super) fn stage_tag(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    tag: &ItemTag<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let mut staged = Vec::new();
    for piece in &tag.pieces {
        match piece {
            WrittenPiece::Node(node) => staged.extend(lower_run(cx, lw, std::slice::from_ref(node))?),
            WrittenPiece::Text(text) if text.is_empty() => {}
            WrittenPiece::Text(text) => staged.push(lw.text(cx, text)?),
        }
    }
    Ok(staged)
}

impl Lower for ListRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<ListElement>(&node)
            .ok_or_else(|| LowerError::structure("a list without its element's data"))?;
        let body = list_items(&node);
        report_reading(&node, value, &body, lw);
        let list_label = self.list_label(&node, value, &body, lw)?;
        // The numbers are read before anything is staged, since reading the
        // table borrows the façade the staging needs.
        let mut numbers: Vec<Option<Vec<NumberSegment>>> = vec![None; body.items.len()];
        if let ListLabel::Numbers(own) = &list_label {
            let read = match lw.get(COUNTERS)? {
                Some(counters) => Some(self.numbers(&body, counters, own.as_deref(), lw)?),
                None => None,
            };
            if let Some(read) = read {
                numbers = read.numbers;
                if read.missing {
                    report_missing_entry(&node, COUNTERS.name(), lw);
                }
            }
        }
        // Each item is a link target, anchored as its reference point is.
        // See decision rationale at [§flm-dd-dr:rend-list-entry-anchors].
        let anchors = item_anchors(&node, &body, lw)?;
        let mut entry_anchors: Vec<Option<String>> = Vec::new();
        let mut slots: Vec<(String, Vec<BuildId>)> = Vec::new();
        if !body.before.is_empty() {
            entry_anchors.push(None);
            slots.push((LABEL_SLOT.to_string(), Vec::new()));
            slots.push((ITEM_SLOT.to_string(), lower_run(cx, lw, &body.before)?));
        }
        entry_anchors.extend(anchors);
        for (item, number) in body.items.iter().zip(numbers) {
            let label = match (&item.tag, &list_label) {
                (Some(tag), _) => ItemLabel::Tag(tag),
                (None, ListLabel::Numbers(_)) => number.map_or(ItemLabel::Nothing, ItemLabel::Number),
                (None, ListLabel::Fixed(tree)) => ItemLabel::Tree(Arc::clone(tree)),
                // An item with no tag where the list writes no label has been
                // reported with the list's writing, or its list's own label
                // could not be used and has been reported.
                (None, ListLabel::Nothing | ListLabel::Missing) => ItemLabel::Nothing,
            };
            let label = ListRule::stage_label(cx, lw, label)?;
            slots.push((LABEL_SLOT.to_string(), label));
            slots.push((ITEM_SLOT.to_string(), lower_run(cx, lw, &item.content)?));
        }
        let payload = ListPayload { kinds: kind_tokens(value), anchor: None, entry_anchors };
        let list = lw.pres_slots_with(cx, &ElementName::from(pres::LIST), payload, slots)?;
        Ok(Lowering::Emit(vec![list]))
    }
}

/// The lowering rule [`ITEM_RULE`](super::ITEM_RULE) of an `\item`, the
/// default on every backend. A list's rule lowers the content of its items
/// and never an `\item` node, so an `\item` that the lowering reaches on its
/// own is one that begins no item. The rule writes nothing for it and reports
/// it as [`ItemOutsideList`](super::diagnostics::ItemOutsideList).
#[derive(Debug, Default)]
pub struct ItemRule;

impl Lower for ItemRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.diagnose(Diagnostic::error(ItemOutsideList, node.span().clone()));
        Ok(Lowering::Emit(Vec::new()))
    }
}
