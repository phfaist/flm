//! The recomposition rule of the `lists` feature for the `latex-recomposed`
//! backend ([`LATEX_RULE`], [`ListLatexRule`]): it writes a list back as its
//! LaTeX environment, under the list kind's name, and each item as `\item`.
//! The rule is the default on the `latex-recomposed` backend only; the
//! [rendering guide](crate::guide::rendering#lists-in-the-recomposed-latex)
//! describes the output for authors.
//!
//! | written | LaTeX |
//! |---|---|
//! | `\begin{<kind>}` | `\begin{<kind>}`: LaTeX sets the labels beside the items |
//! | `\begin{<kind>}[label=(#{alph})]`, `[(a)]` | `\begin{<kind>}[label={(\alph*)}]`, with a package request for `enumitem` |
//! | `\begin{<kind>}[label=#{arabic}., ref=#{arabic}]`, `[1.]` | `\begin{<kind>}[label={\arabic*.}, ref={\arabic*}]` |
//! | `\begin{<kind>}[ref=(#{alph})]` | `\begin{<kind>}[ref={(\alph*)}]` |
//! | `\begin{<kind>}[label={--}]`, `[-]`, `[*]`, `[+]` | `\begin{<kind>}[label={--}]`, `[label={-}]`, … |
//! | `\begin{<kind>}[label={Step~#{arabic}:}]`, `[label={{Q}#{arabic}}]` | `\begin{<kind>}[label={Step~\arabic*:}]`, `[label={{Q}\arabic*}]` |
//! | `\begin{<kind>}[label={\textbf{#{arabic}}.}]` | `\begin{<kind>}[label={\textbf{\arabic*}.}]` |
//! | `\begin{<kind>}[label=#{fnsymbol}]` | `\begin{<kind>}`, reported: `enumitem` has no command for the number format |
//! | `\item` | `\item` followed by a space |
//! | `\item[My tag]`, `\item[tag=My tag]` | `\item[My tag]`, the tag as the document wrote it |
//! | `\item[tag={My tag}]` | `\item[{My tag}]`, with the braces the document wrote |
//! | `\label{item:x}` after an `\item` | inside the LaTeX hook `\flmLabels` (see below) after the `\item`, as every label is written |
//!
//! A LaTeX hook ([LaTeX hooks](crate::stdcore::backends::latex::preamble)) is
//! a macro that the recomposed LaTeX writes and the style file defines with a
//! plain default, which a user may redefine; `\flmLabels` is the one
//! written around every group of labels.
//!
//! # A list's own label template
//!
//! A list's own label template (its `label=` key or its short form) is
//! written as the `label=` key of LaTeX's `enumitem` package, and the rule
//! records a package request for that package ([`ENUMITEM`]; see
//! [package requests](crate::stdcore::backends::latex::packages)). The
//! template's content is recomposed as any content of the document is,
//! through the ordinary rules: text escaped, `~` and the other escaped
//! characters as this output writes them everywhere, a command such as
//! `\textbf{…}` as its rule writes it, a formula as every formula, a user
//! definition as every use of one. Each number format in the template is
//! written as the package's command for it, wherever it occurs in the
//! content (`\textbf{\arabic*}`): `#{arabic}` as `\arabic*`, `#{alph}` as
//! `\alph*`, `#{Alph}` as `\Alph*`, `#{roman}` as `\roman*`, `#{Roman}` as
//! `\Roman*`. The value is written as one brace group, `label={…}`, so that
//! a comma, an equals sign or a closing bracket in it belongs to the label.
//!
//! The key is not written, and the problem is reported, in two cases:
//!
//! - the template writes a number format for which the package has no
//!   command, such as `#{fnsymbol}`, which the numbering feature can write
//!   ([`NoEnumitemFormat`]);
//! - the template writes a number format inside content whose characters are
//!   read as written, such as verbatim content ([`NumberReadAsCharacters`]):
//!   the package's command is LaTeX that runs as each label is set, and
//!   verbatim characters are read before that.
//!
//! A template that does not compile, and a template that writes a number on
//! an unnumbered list, are reported as the general rule reports them, and
//! the list is written without `label=`: LaTeX then sets its labels, as the
//! depth's template does on other backends.
//!
//! The rule compiles the templates of the optional argument from their
//! source text. A list inside the template of a kept definition (a user
//! definition that the recomposed LaTeX writes as a LaTeX macro or
//! environment; see [`kept`](crate::stdcore::features::userdefs::kept)),
//! whose optional argument holds a placeholder of that template
//! (`\begin{enumerate}[label={(#1)}]`), therefore cannot be written inside
//! the kept definition:
//! the rule returns [`LowerError::PlaceholderCharacters`]
//! ([`spelling_at_lowering`]), and each use site of the definition is
//! expanded instead, its list written with the label the call gives.
//!
//! # A numbered list's own reference template
//!
//! A numbered list's own reference template (its `ref=` key, or the one its
//! short form gives) is written as the package's `ref=` key, in the same way,
//! beside `label=` where it differs from the label template. Where the two
//! are the same text, as `label=` alone makes them, only `label=` is written,
//! as the author wrote one key, and the package uses it for references to
//! the items too. A reference template that does not compile is reported as
//! on every other backend and not written. An unnumbered list's `ref=` is
//! ignored, as on every backend ([`RefIgnored`](super::RefIgnored)), and not
//! written. A numbered list in a registry without the numbering feature is
//! reported as on every other backend ([`NoNumbering`](super::NoNumbering))
//! and still written: LaTeX numbers its items.
//!
//! # The labels of a list kind
//!
//! The label templates a list kind declares for each depth are not written:
//! the environment is LaTeX's own, and LaTeX sets the labels beside its
//! items, as this output leaves every other choice of presentation to LaTeX.
//! LaTeX's labels for `enumerate` are `1.`, `(a)`, `i.` and `A.`, where the
//! list kind's default in FLM writes `1.`, `(i)`, `a-`. A list environment
//! that a program embedding the library configures is written under its own
//! name, which the author's LaTeX must define.
//!
//! # Content, and registration
//!
//! The content of every item, and any content before the first `\item`, is
//! written through the ordinary rules, so text is escaped and a reference
//! inside an item is written as every reference is. The rule is registered
//! under peer declarations ([`PeerName`](crate::core::registry::PeerName)) of
//! the `latex-recomposed` backend, of the constructs it stages and of the
//! safe-labels table it reads. A registry without the LaTeX
//! features therefore still builds, with the rule set aside (excluded from
//! rule selection; see [`PeerName`](crate::core::registry::PeerName)).

use std::sync::Arc;

use techy::core::node::{BuildId, GroupData, NodeKind, NodeRef, NodeTree};
use techy::core::Lang;
use techy::error::Diagnostic;
use techy::source::SourceSpan;
use techy::transform::{Restage, RestageContext, RestageVisitor};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    spelling_at_lowering,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::flm::FlmCallableType;
use crate::lang::guard::{self, on_worker_thread, GuardPolicy};
use crate::lang::rewrites::userdefs::{CompiledTemplate, PlaceholderTarget};
use crate::lang::{Flm, FlmGroupType};
use crate::model::aspects::is_whitespace_of;
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{stamp_point, ARG_SLOT, ENVIRONMENT, MACRO, RAW};
use crate::stdcore::backends::latex::packages::ENUMITEM;
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexEnvironmentPayload, LatexMacroPayload, LatexRawPayload,
};
use crate::stdcore::backends::latex_recomposed::{LatexRecomposedRenderOptions, BACKEND};
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::backends::pres::BODY_SLOT;
use crate::stdcore::features::numbering::{occurrences_of, COUNTERS};
use crate::stdcore::features::refs::{self, SafeLabels, SAFE_LABELS};

use super::diagnostics::{NoEnumitemFormat, NumberReadAsCharacters};
use super::items::{is_item, list_items};
use super::labels::{CompiledLabel, Labels};
use super::lower::{own_label, own_ref, report_no_numbering, report_reading, stage_tag};
use super::{ListElement, ITEM_COMMAND, LABEL_KEY, LIST, OPTIONS_ARG, REF_KEY};

/// The name of the recomposition rule, `flm.lists.list.lower.latex-recomposed`.
pub const LATEX_RULE: &str = "flm.lists.list.lower.latex-recomposed";

/// The recomposition rule [`LATEX_RULE`] of a list, the default on the
/// `latex-recomposed` backend. It writes the list back as its environment:
/// the list's own label template as the `enumitem` package's `label=` key,
/// its own reference template, where it differs, as `ref=`, each item as
/// `\item` with its tag as the command's optional argument and its labels
/// in the LaTeX hook `\flmLabels`
/// ([LaTeX hooks](crate::stdcore::backends::latex::preamble)) after it, and the items' content through
/// the ordinary rules. It reports the mistakes in how the list is written
/// and the problems of the labels beside its items as the general rule does
/// ([`ListRule`](super::ListRule)), and the parts of its own templates that
/// it cannot write. See the [module documentation](self).
///
/// The feature creates the rule and gives it the compiled templates and the
/// template language in which a list's own templates are compiled.
#[derive(Debug)]
pub struct ListLatexRule {
    labels: Arc<Labels>,
}

impl ListLatexRule {
    /// The rule over `labels`: the feature's compiled templates and the
    /// template language in which a list's own templates are compiled.
    pub(super) fn new(labels: Arc<Labels>) -> ListLatexRule {
        ListLatexRule { labels }
    }
}

/// The five number formats the `enumitem` package can write in a `label=`
/// or `ref=` value, each as a template writes its name, with the package's
/// command for the item's number in that format.
const ENUMITEM_FORMATS: [(&str, &str); 5] = [
    ("arabic", "\\arabic*"),
    ("alph", "\\alph*"),
    ("Alph", "\\Alph*"),
    ("roman", "\\roman*"),
    ("Roman", "\\Roman*"),
];

/// The `enumitem` command for the number format that a template writes as
/// `#{name}`, where the package has one.
fn enumitem_command(name: &str) -> Option<&'static str> {
    ENUMITEM_FORMATS.iter().find(|(format, _)| *format == name).map(|(_, command)| *command)
}

/// The visitor that copies a list's own template for its `enumitem` key,
/// called once, at the template tree's root: it emits a list node holding
/// one brace group around the copies of the root's children. In the copy,
/// every placeholder of a number format becomes a
/// [`LatexPoint`](crate::stdcore::backends::latex::LatexPoint) node (the
/// LaTeX text a copy of a template writes at one of its placeholders)
/// holding the package's command for that format; every other node is
/// copied unchanged.
struct Braced<'t> {
    template: &'t CompiledTemplate,
}

/// The visitor [`Braced`] copies the root's children with.
struct Stamp<'t> {
    template: &'t CompiledTemplate,
}

impl RestageVisitor<Flm, (), ()> for Stamp<'_> {
    type Error = LowerError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, LowerError> {
        if node.callable_type() != Some(FlmCallableType::Placeholder) {
            return Ok(Restage::Descend(()));
        }
        let command = match self.template.placeholder(node.id()).map(|reference| &reference.target) {
            Some(PlaceholderTarget::Extra(format)) => enumitem_command(format),
            _ => None,
        };
        let command = command.ok_or_else(|| {
            LowerError::structure("a placeholder of a list's label that is not an enumitem format")
        })?;
        Ok(Restage::Emit(vec![stamp_point(&node, command, cx)?]))
    }
}

impl RestageVisitor<Flm, (), ()> for Braced<'_> {
    type Error = LowerError;

    fn restage(
        &mut self,
        root: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, LowerError> {
        let span = SourceSpan::new(root.span().source(), root.span().start()..root.span().start());
        let state = Arc::clone(root.parsing_state());
        let mut stamp = Stamp { template: self.template };
        let mut children = Vec::new();
        for node in root.children().iter() {
            children.extend(cx.restage_subtree(node, &mut stamp).map_err(copy_failed)?);
        }
        let mut add = |kind: NodeKind<Flm>, children: Vec<BuildId>| {
            let ext =
                Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&children))
                    .map_err(copy_failed)?;
            cx.builder()
                .add(kind, span.clone(), Arc::clone(&state), children, ext, ())
                .map_err(copy_failed)
        };
        let group = add(NodeKind::group(GroupData::new(FlmGroupType::Content, "{", "}")), children)?;
        Ok(Restage::Emit(vec![add(NodeKind::list(), vec![group])?]))
    }
}

/// The failure of the copy of a list's template, as a lowering error.
fn copy_failed(error: impl std::fmt::Display) -> LowerError {
    LowerError::other(format!("copying a list's label failed: {error}"))
}

/// The content of `label` as the value of an `enumitem` key: a tree holding
/// one brace group around a copy of the template's content, each number
/// format it writes replaced by the package's command for it, ready to be
/// lowered through the ordinary rules. Every number format the template
/// writes must be one of [`ENUMITEM_FORMATS`]; the caller checks this.
///
/// # Errors
///
/// The failure of the copy: the content nests too deeply, or no worker
/// thread could be made for it.
fn label_value(label: &CompiledLabel) -> Result<Arc<NodeTree<Flm>>, LowerError> {
    let template = label.template();
    // The copy descends recursively: it runs on a worker thread, which the
    // lowering stage's own thread already is.
    on_worker_thread(|| {
        guard::restager(&mut Braced { template }, GuardPolicy::StackBudget)
            .restage(template.tree())
            .map(Arc::new)
            .map_err(copy_failed)
    })
    .map_err(|error| LowerError::other(format!(
        "copying a list's label template needs a thread with a large stack, and the \
         operating system refused to create one: {error}"
    )))?
}

/// Stages a raw node (`flm.pres.latex.raw`) holding the LaTeX text `text`.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Stages one key of the list's optional argument, `key` (`label` or
/// `ref`), writing the compiled template `label`, whose text is `template`:
/// `key={…}`, the value being the template's content lowered through the
/// ordinary rules; it records a package request for `enumitem`. A template
/// that writes a number format for which the package has no command is
/// reported as [`NoEnumitemFormat`], and one that writes a number format
/// where characters are read as [`NumberReadAsCharacters`]; in either case
/// the key is not written, and the function returns `None`.
fn stage_label_key(
    node: &NodeRef<'_, Flm>,
    key: &str,
    template: &str,
    label: &CompiledLabel,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Option<Vec<BuildId>>, LowerError> {
    let unwritable = occurrences_of(label.template())
        .into_iter()
        .find(|(format, _)| enumitem_command(format).is_none());
    if let Some((format, _)) = unwritable {
        lw.diagnose(Diagnostic::error(
            NoEnumitemFormat { key: key.to_string(), template: template.to_string(), format },
            node.span().clone(),
        ));
        return Ok(None);
    }
    // The key is LaTeX's key-value syntax, a raw piece; the value is the
    // template's content, lowered through the rules as any content is, in a
    // brace group, so that a comma or a bracket in it is the value's.
    // See decision rationale at [§flm-dd-dr:std-list-recomposed-labels].
    let value = label_value(label)?;
    // A number format where the content's characters are read — inside
    // verbatim content — has no LaTeX to stand for it.
    // See decision rationale at [§flm-dd-dr:rend-kept-placeholder-characters].
    let content = match lw.splice(cx, &value) {
        Err(LowerError::PlaceholderCharacters) => {
            let condition =
                NumberReadAsCharacters { key: key.to_string(), template: template.to_string() };
            lw.diagnose(Diagnostic::error(condition, node.span().clone()));
            return Ok(None);
        }
        other => other?,
    };
    let mut staged = vec![raw(cx, lw, &format!("{key}="))?];
    staged.extend(content);
    lw.require(&Name::from(ENUMITEM));
    Ok(Some(staged))
}

impl Lower for ListLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<ListElement>(&node)
            .ok_or_else(|| LowerError::structure("a list without its element's data"))?;
        // The bracket's templates are compiled from their spelling: see
        // [`spelling_at_lowering`].
        if let Some(options) = node.argument_content_nodes_named(OPTIONS_ARG).ok().flatten() {
            spelling_at_lowering(options.iter())?;
        }
        let body = list_items(&node);
        report_reading(&node, value, &body, lw);
        // A numbered list's own reference template is compiled and reported by
        // the numbering stage where there is one, as on every other backend;
        // the rule compiles it again to write it, and reports nothing then.
        let counted = value.kind.numbered && lw.get(COUNTERS)?.is_some();
        let own = own_label(&self.labels, &node, value, &body, counted, lw);
        let reference = own_ref(&self.labels, &node, value, &body, counted, lw);
        // A numbered list nothing numbers is reported as on every other
        // backend, and written all the same: LaTeX numbers it here, as it
        // resolves everything else in this output.
        report_no_numbering(&self.labels, &node, value, &body, own.as_ref(), lw)?;
        let mut shapes = Vec::new();
        let mut slots = Vec::new();
        // One key where the author wrote one template for both, two where the
        // reference template differs from the label template.
        // See decision rationale at [§flm-dd-dr:std-list-recomposed-labels].
        let label_text = body.own_label.as_ref().map(|own| own.template.as_str());
        let ref_text = body.own_ref.as_deref().filter(|text| Some(*text) != label_text);
        let mut keys = Vec::new();
        if let (Some(label), Some(text)) = (&own, label_text) {
            keys.extend(stage_label_key(&node, LABEL_KEY, text, label, cx, lw)?);
        }
        if let (Some(label), Some(text)) = (&reference, ref_text) {
            keys.extend(stage_label_key(&node, REF_KEY, text, label, cx, lw)?);
        }
        if !keys.is_empty() {
            let mut staged = Vec::new();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    staged.push(raw(cx, lw, ", ")?);
                }
                staged.extend(key);
            }
            shapes.push(LatexArgShape::Optional);
            slots.push((ARG_SLOT.to_string(), staged));
        }
        let mut content: Vec<BuildId> = Vec::new();
        // What stands before the first `\item`: content, an error reported
        // above, is written as it stands; whitespace and comments alone are
        // written without the nodes that are whitespace, such as `~`, which
        // LaTeX would read as text before the first `\item`.
        let before: Vec<NodeRef<'_, Flm>> = node
            .body()
            .map(|children| children.iter().take_while(|child| !is_item(child)).collect())
            .unwrap_or_default();
        if body.before.is_empty() {
            for run in before.split(is_whitespace_of) {
                content.extend(lower_run(cx, lw, run)?);
            }
        } else {
            content.extend(lower_run(cx, lw, &before)?);
        }
        for item in &body.items {
            let (args, item_slots) = match &item.tag {
                None => (Vec::new(), Vec::new()),
                Some(tag) => {
                    let staged = stage_tag(cx, lw, tag)?;
                    (vec![LatexArgShape::Optional], vec![(ARG_SLOT.to_string(), staged)])
                }
            };
            content.push(lw.pres_slots_with(
                cx,
                &ElementName::from(MACRO),
                LatexMacroPayload::new(ITEM_COMMAND, args),
                item_slots,
            )?);
            let mut labels = Vec::new();
            for written in &item.labels {
                labels.push(refs::stage_label(cx, lw, &written.text, written.label.as_ref())?);
            }
            content.extend(refs::stage_label_group(cx, lw, labels, false)?);
            content.extend(lower_run(cx, lw, &item.content)?);
        }
        slots.push((BODY_SLOT.to_string(), content));
        let environment = lw.pres_slots_with(
            cx,
            &ElementName::from(ENVIRONMENT),
            LatexEnvironmentPayload::new(value.name.as_str(), shapes),
            slots,
        )?;
        Ok(Lowering::Emit(vec![environment]))
    }
}

/// The recomposition rule of a list, [`LATEX_RULE`]: [`ListLatexRule`] over
/// `labels`, the default on the `latex-recomposed` backend only.
pub(super) fn latex_rule(labels: Arc<Labels>) -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        LIST,
        LoweringDecl::new()
            .reads_opt(SAFE_LABELS)
            .reads_opt(COUNTERS)
            .option::<LatexRecomposedRenderOptions>()
            .produces(ENVIRONMENT)
            .produces(MACRO)
            .produces(RAW),
        Arc::new(ListLatexRule::new(labels)),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a list lowers to its LaTeX environment, its own templates as the `label=` and `ref=` \
         keys of the `enumitem` package, each item to `\\item` with its tag as the optional \
         argument and its labels in the LaTeX hook `\\flmLabels`, and the items' content through \
         the rules",
    )
}

/// Registers what the feature contributes to the `latex-recomposed` backend:
/// the peer declarations ([`PeerName`](crate::core::registry::PeerName)) of
/// the backend, of the constructs the rule stages and of the safe-labels
/// table it reads, then the rule of a list, over `labels`.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made in
/// a registration round that does not accept it (see
/// [`FeatureBuilder`](crate::core::registry::FeatureBuilder)).
pub(super) fn register(b: &mut FeatureBuilder<'_>, labels: Arc<Labels>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rule of a list is the default",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        ENVIRONMENT,
        "the LaTeX family's environment construct, which the recomposition rule stages for the list",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages for `\\item` \
         and the LaTeX hook `\\flmLabels`",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, in which the recomposition rule stages the keys of a \
         list's own templates and a label written as the document wrote it",
    )?;
    b.peer_table::<SafeLabels>(
        SAFE_LABELS.name(),
        "the references feature's safe-labels table, which the recomposition rule reads for the \
         safe label of each label written after an `\\item`",
    )?;
    b.lowering_rule(latex_rule(labels))
}
