//! The `lists` feature ([`Lists`]): list environments whose items are begun
//! by `\item`, the label shown beside each item, and references to items. By
//! default the list environments are `itemize`, `enumerate` and
//! `description`.
//!
//! # Lists and list kinds
//!
//! A **list** is an environment whose body is a sequence of items, each begun
//! by `\item`. Each list environment has one **list kind**, which the
//! feature's configuration, [`ListsConfig`], declares under the environment's
//! name: for each list kind, whether its items are numbered and which label is shown beside them
//! at each nesting depth ([`ListKind`]). The default configuration declares
//! three list kinds:
//!
//! ```text
//! \begin{itemize}          \begin{enumerate}        \begin{description}
//! \item First              \item First              \item[Term] What it means.
//! \item Second             \item Second             \item[Other] More.
//! \end{itemize}            \end{enumerate}          \end{description}
//!
//! • First                  1. First                 Term  What it means.
//! • Second                 2. Second                Other More.
//! ```
//!
//! A list nested in an item of another list of the same list kind is at the
//! next **nesting depth**, and shows the list kind's next label template:
//! `-` then `▸` for `itemize`, `(i)` then `a-` for `enumerate`; the last
//! template serves every deeper list. Only lists of the same list kind are
//! counted: an `enumerate` inside an `itemize` inside an `enumerate` is at
//! depth 2 and shows `(i)`, as in LaTeX. The
//! [language guide](crate::guide::language_syntax#lists) describes lists for
//! authors.
//!
//! # The optional argument of a list
//!
//! Each list environment takes an optional argument, a key-value argument
//! with two keys:
//!
//! - `label` is the list's own **label template**, used beside its items in
//!   place of the depth's template. It is written in the language of a
//!   counter style's number template: every `#{name}` is the item's number in
//!   the number format `name`, and every other character is FLM text written
//!   as is.
//! - `ref` is the list's own **reference template**, in the same language:
//!   the template through which a reference to one of its items writes the
//!   item's number, in place of the depth's reference template. The key is
//!   named after the key of LaTeX's `enumitem` package that does the same.
//!
//! `label=` written alone sets both templates to the same text:
//!
//! ```text
//! \begin{enumerate}[label=(#{alph})]                % (a), (b); \ref: Item~(b)
//! \begin{enumerate}[label=#{arabic}:, ref=#{arabic}] % 1:, 2:;   \ref: Item~2
//! \begin{itemize}[label={--}]                        % a dash beside every item
//! ```
//!
//! A bracket holding no `=` and made only of the characters `[]().:-+*aAiI1`
//! is a **short form** of `label=`, the syntax of LaTeX's `enumerate`
//! package. With one of the characters `a`, `A`, `i`, `I`, `1`, that
//! character stands for the number in the number format it shows: `[(i)]` is
//! `label=(#{roman})`, `[1.]` is `label=#{arabic}.`, `[a-]` is
//! `label=#{alph}-`. With none of them, the text is a fixed label: `[-]`,
//! `[*]`, `[+]`. A short form that writes a number also sets the reference
//! template: the same text with every full stop, colon and hyphen at its end
//! dropped, so `[1.:]` is `label=#{arabic}.:, ref=#{arabic}` and `[(i)]`
//! refers to its items as `(#{roman})`. A fixed short form refers to its
//! items by its label. Any other text, a Unicode bullet included, is written
//! with `label={…}`; a `]` inside the bracket is protected by braces. A short
//! form holding two of the five characters is an error, since it could mean
//! two things ([`BadLabelForm`]). See [`list_options`], [`short_form`] and
//! [`short_form_ref`].
//!
//! A `ref=` on a list whose list kind is not numbered has no number to
//! write. It is accepted and reported as a Note ([`RefIgnored`]), so that a
//! list can be switched between a numbered and an unnumbered list kind
//! without its bracket being rewritten.
//!
//! # Items
//!
//! Inside a list's body, and nowhere else, `\item` begins an item. `\item`
//! is declared in a scoped package ([`ScopedPackageDef`]), [`PACKAGE`]:
//! a set of element declarations that can be written only inside the bodies
//! and arguments whose declarations refer to the package (see
//! [scoped packages](crate::guide::concepts_overview#scoped-packages)); the
//! body of every list environment refers to it. Everything after an `\item`
//! up to the next one, or to the end of the body, is the item's content,
//! which is block content: paragraphs, nested lists, displayed formulas. An
//! empty item is still an item. Whitespace and comments before the first
//! `\item` are skipped; any other content there is the error
//! [`ContentBeforeItem`].
//!
//! An `\item` takes an optional key-value argument with one key, `tag`: FLM
//! content shown as the item's label in place of the list's template, in a
//! numbered list as in an unnumbered one. A bracket holding no `=` outside
//! braces is the whole tag: `\item[My tag]` is `\item[tag={My tag}]`, and
//! `\item[tag={a=b}]` protects a `=`. See [`item_options`].
//!
//! After an `\item` and its bracket, `\label{item:…}` fields attach labels to
//! the item, several of them if needed, as they do after a heading's title.
//! A label is the text an author writes as `\label{type:label}` after a
//! construct (see
//! [labels](crate::guide::concepts_overview#reference-points-labels-and-anchors));
//! an item accepts the label type `item` only ([`LABEL_TYPES`]).
//!
//! # The label beside an item
//!
//! The label shown beside an item is, in this order: its tag; else the
//! list's own label template; else the label template the list kind declares
//! for the list's depth.
//!
//! - A template that writes a number writes the item's number through that
//!   label template, without the word a reference writes before it: `2.`
//!   beside the item, `Item~2` in a reference. The numbering feature's stage
//!   [`NUMBER`](crate::stdcore::features::numbering::NUMBER), which records
//!   each reference point's counter value in the counter table, records each
//!   item's number with the counter style of the list's depth, whose number
//!   format is the list's reference template. To write the label, the list's
//!   rule takes that counter style, replaces its number format by the label
//!   template
//!   ([`CounterStyle::with_number_format`](crate::stdcore::features::numbering::CounterStyle::with_number_format)),
//!   and compiles the result once per list when the list is lowered.
//! - A template that writes no number is a fixed label, shown beside every
//!   item.
//! - A list of a list kind that declares no template (`description`) writes
//!   no label of its own, and an item there without a tag is the error
//!   [`MissingTag`].
//!
//! A fixed template on a numbered list (`\begin{enumerate}[label={*}]`)
//! shows `*` beside every item. Every item is still counted and has its
//! number, which a reference writes through the same template, `Item~*`,
//! since `label=` alone sets the reference template too. An explicit empty
//! `label={}` shows no label at all. Whether a list is numbered depends on
//! its list kind, never on its template: a template that writes a number on
//! an unnumbered list is the error [`LabelNamesNumber`], and the depth's
//! template is used instead.
//!
//! # Numbers
//!
//! The items of a numbered list kind are numbered by the
//! [`numbering`](crate::stdcore::features::numbering) feature, and by nothing
//! else. Each numbered list kind declares a counter
//! ([`CounterDecl`](crate::stdcore::features::numbering::CounterDecl)) and
//! one counter style
//! ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl))
//! per depth, whose number format is the depth's reference template
//! ([`numbers`]). Each list environment of the list kind is a scoped instance
//! of the counter (see
//! [scoped instances](crate::model::aspects::NumberSource#scoped-instances)
//! and, for authors, the
//! [numbering guide](crate::guide::numbering#scoped-instances)), so that
//! each list numbers its own items from one, a list nested in a numbered item
//! of another list is counted within that item, and the enclosing list
//! continues after it.
//!
//! An item carrying a tag is counted in no counter and has no number:
//! `\item[Custom] \item \item` shows `Custom`, `1.`, `2.`, as in LaTeX.
//!
//! In a registry without the numbering feature, nothing numbers items. The
//! counters and counter styles are dropped when the registry is built, with
//! one Note for each of the numbering feature's two service slots, and a
//! numbered list whose label writes a number shows
//! no label and is reported once ([`NoNumbering`]); a fixed label and a tag
//! are still shown.
//!
//! # References to items
//!
//! Every `\item` of a list is one reference point: one place in a document
//! that can be referred to, numbered, or given an anchor
//! ([`RefPoint`]; see
//! [reference points](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//! Its reference kind is `item` ([`KIND`]), and its anchor is derived from
//! its first label or, for an unlabeled item, from its position
//! ([`ANCHOR_PREFIX`]). `\ref{item:x}` shows what is shown beside the item,
//! through the ordinary path of the
//! [references](crate::stdcore::features::refs) feature:
//!
//! - for a numbered item, the word and the number written through the list's
//!   reference template: `Item~2`, `Item~(ii)`, and `(ii)` under
//!   `\ref[noprefix]`;
//! - for an item carrying a tag, the tag, however it is written
//!   (`\item[Custom]`, `tag=Custom`, `tag={Custom}`);
//! - for an untagged item of an unnumbered list, its label: the list's own
//!   (`[-]`, `label=*`, `label={--}`) where it writes one that can be used,
//!   else the label template the list kind declares for the depth, so that
//!   `\ref{item:x}` on an `itemize` item shows `•`.
//!
//! An unnumbered list kind's reference templates, and a `ref=` on its lists,
//! have no effect. [`ItemElement`] describes how the reference point carries
//! that content.
//!
//! # What the feature registers
//!
//! Under a [`ListsConfig`], the feature registers:
//!
//! - one value of the element type [`LIST`] (`flm.lists.list`) per list kind,
//!   written as the environment of the list kind's name, carrying a
//!   [`ListElement`];
//! - the scoped package [`PACKAGE`] holding `\item`, whose element [`ITEM`]
//!   (`flm.lists.item`) carries an [`ItemElement`], the item's reference
//!   point;
//! - for each numbered list kind, a counter and its counter styles, as peer
//!   contributions to the numbering feature's service slots ([`numbers`]);
//! - peer declarations ([`PeerName`](crate::core::registry::PeerName)) of the
//!   numbering feature's counter table and of the references feature's
//!   anchors table, which the list rule reads where they exist, and of the
//!   references feature's argument role
//!   [`DEFINED_LABEL_ARG_ROLE`](crate::stdcore::features::refs::DEFINED_LABEL_ARG_ROLE),
//!   the role of the argument of an item's `\label` field;
//! - a build hook ([`LateInit`](crate::core::registry::LateInit)) that
//!   compiles every list kind's label templates, and a numbered list kind's
//!   reference templates, once the registry's parsing language
//!   ([`BuiltLang`](crate::lang::BuiltLang)) is built; a template that does
//!   not compile, or a label template of an unnumbered list kind that writes
//!   a number format, fails the registry build;
//! - the lowering rules [`RULE`] and [`ITEM_RULE`], and the recomposition
//!   rule [`latex::LATEX_RULE`] of the `latex-recomposed` backend;
//! - its settings: the fields of [`ListsConfig`] as data
//!   ([`crate::core::settings`]).
//!
//! # How a list is lowered
//!
//! On every backend except `latex-recomposed`, the lowering rule [`RULE`]
//! ([`ListRule`]) lowers a list to one
//! [`flm.pres.list`](crate::stdcore::backends::pres::LIST) node holding, per
//! item, a `label` content slot and an `item` content slot. Its kind tokens
//! are, in this order:
//!
//! - the list kind's structural token:
//!   [`LIST_NUMBERED`](crate::stdcore::backends::pres::LIST_NUMBERED) for a
//!   numbered list kind,
//!   [`LIST_UNNUMBERED`](crate::stdcore::backends::pres::LIST_UNNUMBERED) for
//!   an unnumbered list kind that declares label templates,
//!   [`LIST_TERMS`](crate::stdcore::backends::pres::LIST_TERMS) for a list
//!   kind that declares none;
//! - the environment's name;
//! - [`LIST_LABELED`](crate::stdcore::backends::pres::LIST_LABELED), since
//!   every item's label is written as content of the `label` slot.
//!
//! Each item's anchor (the identifier its reference point was given) is
//! carried on the list's payload
//! ([`ListPayload`](crate::stdcore::backends::pres::ListPayload)), one per
//! entry, read from the references feature's anchors table where there is
//! one, so that a link to the item leads to it. A list inside a footnote
//! keeps its items' numbers and anchors where the
//! [`xnotes`](crate::stdcore::features::xnotes) feature gathers the
//! footnotes, because the footnotes are shown there with their own content,
//! not with a copy of it.
//!
//! What each backend makes of the `flm.pres.list` node:
//!
//! - **HTML**: an `<ol>`, `<ul>` or `<dl>` chosen by the structural kind
//!   token, as the [HTML contract](crate::stdcore::backends::html::CONVENTIONS)
//!   states; the `labeled` kind token switches off the marker the browser
//!   would draw.
//! - **Text**: each label followed by the item's blocks, every item's blocks
//!   aligned at one column.
//! - **`latex-recomposed`**: the general rule is not used; the recomposition
//!   rule [`LATEX_RULE`](latex::LATEX_RULE) writes the list back as its
//!   environment and its items as `\item`s ([`latex`]).
//!
//! A list inside a
//! [synthesized value](crate::guide::concepts_overview#synthesized-values)
//! (for example a label template that holds a list) has items that the
//! references feature's stage
//! [`ANCHORS_STAGE`](crate::stdcore::features::refs::ANCHORS_STAGE), which
//! gives each reference point its anchor, and the numbering stage never saw.
//! The list is reported once for each table that holds no entry for those
//! items (`flm.run.missing-entry`), and it is still written, those items
//! without an anchor and without a number.
//!
//! [`ITEM_RULE`] ([`ItemRule`]), the default on every backend, lowers an
//! `\item` that is inside a list but not directly in its body, which begins
//! no item, to nothing, and reports [`ItemOutsideList`].
//!
//! # Interactions with other features
//!
//! - The [`numbering`](crate::stdcore::features::numbering) feature numbers
//!   the items of numbered lists; without it, see [Numbers](#numbers).
//! - The [`refs`] feature checks the labels,
//!   computes the anchors and resolves `\ref`; without it, no item has an
//!   anchor, and nothing is reported.
//! - The [`refpoints`](crate::stdcore::features::refpoints) feature collects
//!   the items' reference points.
//! - The LaTeX family (the [`latex`](crate::stdcore::backends::latex)
//!   helpers and the `latex-recomposed` backend) owns the constructs
//!   the recomposition rule stages; without them, that rule is set aside
//!   (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)) and the registry still
//!   builds.
//!
//! # Diagnostics
//!
//! Every condition is reported under `flm.lists.*` by the lowering rules;
//! each is an Error except the Note [`RefIgnored`], and each item's
//! documentation states the recovery: [`UnknownKey`], [`MissingValue`],
//! [`BadLabelForm`], [`BadLabel`], [`LabelNamesNumber`], [`RefIgnored`],
//! [`ContentBeforeItem`], [`MissingTag`], [`NoNumbering`],
//! [`ItemOutsideList`], [`NoEnumitemFormat`], [`NumberReadAsCharacters`].
//!
//! The mistakes in how a list is written (its optional arguments, the
//! content before its first item, an item missing its tag) and the problems
//! of the label beside its items (a template that cannot be used, a numbered
//! list that nothing numbers) are reported by whichever rule lowers the
//! list, so every backend reports them alike, the recomposed LaTeX included.
//! The recomposition rule alone reports [`NoEnumitemFormat`] and
//! [`NumberReadAsCharacters`], since only it writes a list's own templates as
//! LaTeX. A list's own template that does not compile is reported once: the
//! reference template of a numbered list in a registry with the numbering
//! feature by the numbering stage, which compiles it
//! (`flm.numbering.bad-number-format`), and every other template by the
//! list's rule ([`BadLabel`]).
//!
//! # Items of this module
//!
//! - The feature: [`Lists`], [`feature`], [`FEATURE`]; its configuration
//!   [`ListsConfig`], [`ListKind`].
//! - Names: the elements [`LIST`] and [`ITEM`], the command [`ITEM_COMMAND`],
//!   the scoped package [`PACKAGE`], the argument [`OPTIONS_ARG`] and its
//!   keys [`LABEL_KEY`], [`REF_KEY`], [`TAG_KEY`], the reference kind
//!   [`KIND`], the label types [`LABEL_TYPES`], the anchor prefix
//!   [`ANCHOR_PREFIX`], the rules [`RULE`] and [`ITEM_RULE`].
//! - Element values: [`ListElement`], [`ItemElement`].
//! - Reading a list: [`list_items`], [`item_of`], [`is_item`],
//!   [`enclosing_list`], [`nesting_chain`], [`ListBody`], [`ListItem`]; its
//!   optional arguments: [`list_options`], [`item_options`], [`short_form`],
//!   [`short_form_ref`], [`ListOptions`], [`OwnLabel`], [`ItemOptions`],
//!   [`ItemTag`].
//! - Lowering: [`ListRule`], [`ItemRule`]; the recomposed LaTeX: [`latex`].
//! - Numbers: [`numbers`].
//! - Conditions: listed under [Diagnostics](#diagnostics).

mod config;
mod diagnostics;
mod items;
mod labels;
pub mod latex;
mod lower;
pub mod numbers;
mod syntax;

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::settings::SettingsDecl;
use crate::core::lowering::{DefaultMark, LoweringDecl, LoweringRule};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::copy::{ValueContent, ValuePiece, WrittenPiece, copy_pieces_into_value};
use crate::lang::guard::on_worker_thread;
use crate::lang::Flm;
use crate::lang::mint::{ArgDef, ArgStdKind, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef};
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole,
    NumberSource, RefPoint, Referenceable,
};
use crate::model::element::{element_value, ElementSpec};
use crate::model::names::{Name, RefKind};
use crate::stdcore::backends::pres;
use crate::stdcore::features::numbering::{Counters, COUNTERS};
use crate::stdcore::features::refs;

pub use config::{ListKind, ListsConfig};
pub use diagnostics::{
    BadLabel, BadLabelForm, ContentBeforeItem, ItemOutsideList, LabelNamesNumber, MissingTag,
    MissingValue, NoEnumitemFormat, NoNumbering, NumberReadAsCharacters, RefIgnored,
    UnknownKey,
};
pub use items::{enclosing_list, is_item, item_of, list_items, nesting_chain, ListBody, ListItem};

use items::instance_chain;
pub use lower::{ItemRule, ListRule};
pub use syntax::{
    item_options, list_options, short_form, short_form_ref, ItemOptions, ItemTag, ListOptions,
    OwnLabel,
};

pub(crate) use labels::holds_placeholder;
use labels::{CompileLabels, Labels};

/// The feature's registered name, `lists`.
pub const FEATURE: &str = "lists";

/// The declaration of the feature's settings: the fields of [`ListsConfig`]
/// are its keys. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<ListsConfig>(
            "The list environments: for each list kind, its environment name, whether its items \
            are numbered, its label templates and its reference templates (read for a numbered \
            list kind) by nesting depth, and its nesting joiner. \
            Fixed when the registry is built: it cannot vary per document or per render.",
        )
        .merge_entries("environments", || serde_json::to_value(&ListsConfig::default().environments).map_err(|error| error.to_string()))
        .markup("environments.*.labels")
        .markup("environments.*.ref_labels")
        .markup("environments.*.nesting.joined")
        .markup("environments.*.nesting.template")
}

/// The element type name shared by every list kind, `flm.lists.list`. Each
/// list kind is one element value of this type, carrying a
/// [`ListElement`].
pub const LIST: &str = "flm.lists.list";

/// The element of `\item`, `flm.lists.item`, carrying an [`ItemElement`].
pub const ITEM: &str = "flm.lists.item";

/// The name of the command that begins an item, `item`.
pub const ITEM_COMMAND: &str = "item";

/// The name of the scoped package ([`ScopedPackageDef`]) that holds
/// `\item`, `flm.lists.items`. The body of every list environment refers to
/// it, so `\item` can be written inside a list's body and nowhere else.
pub const PACKAGE: &str = "flm.lists.items";

/// The name of the optional key-value argument of a list environment and of
/// an `\item`, `options`.
pub const OPTIONS_ARG: &str = "options";

/// The key of a list environment's optional argument that gives the list's
/// own label template, `label`.
pub const LABEL_KEY: &str = "label";

/// The key of a list environment's optional argument that gives the list's
/// own reference template, `ref`: the name the LaTeX package `enumitem` gives
/// the same key.
pub const REF_KEY: &str = "ref";

/// The only key of an `\item`'s optional argument, `tag`: the item's tag.
pub const TAG_KEY: &str = "tag";

/// The reference kind of an item's reference point, `item`.
pub const KIND: &str = "item";

/// The label types an item accepts: `item` only. `\label{item:first}`
/// labels an item; a label of another type written after an `\item` defines
/// nothing and is reported by the label check as
/// `flm.refs.label-type-not-accepted`.
pub const LABEL_TYPES: [&str; 1] = ["item"];

/// The prefix of an item's anchor, `item`: an unlabeled item's anchor is
/// `item--<position><four letters>`, a labeled item's anchor is
/// `item-<label>` (see [`ANCHORS`](crate::stdcore::features::refs::ANCHORS)).
pub const ANCHOR_PREFIX: &str = LABEL_TYPES[0];

/// The name of the general lowering rule of the element [`LIST`],
/// `flm.lists.list.lower.default` ([`ListRule`]), the default on every
/// backend except `latex-recomposed`.
pub const RULE: &str = "flm.lists.list.lower.default";

/// The name of the lowering rule of the element [`ITEM`],
/// `flm.lists.item.lower.default` ([`ItemRule`]): the rule of an `\item`
/// that begins no item.
pub const ITEM_RULE: &str = "flm.lists.item.lower.default";

/// The data of one element value of [`LIST`]: the list kind it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListElement {
    /// The list kind's name, which is the environment's name.
    pub name: Name,
    /// What the configuration declares about the list kind.
    pub kind: ListKind,
}

/// The data of the [`ITEM`] element, and its [`Referenceable`] aspect, which
/// declares the reference point of the item an `\item` begins.
///
/// It holds the templates the feature compiled when the registry was built.
/// For an untagged item of an unnumbered list that writes no usable label of
/// its own, the reference point carries as its content the list kind's
/// fixed label template for the depth (the bullet), as the tree compiled
/// once for that template.
#[derive(Debug, Clone)]
pub struct ItemElement {
    labels: Arc<Labels>,
}

/// Declares one reference point per `\item` that begins an item of a list,
/// and none for any other `\item`. The point's labels are the `\label`
/// fields written after the `\item`, of the label type `item`; its anchor is
/// derived from its first label or, for an unlabeled item, from its
/// position.
///
/// An untagged item of a numbered list kind declares a number source
/// ([`NumberSource`]): the list kind's counter; the scoped instances under
/// which it is counted (below), outermost first; the counter style of its
/// list's depth, whose number format is the depth's reference template; the
/// advance of level 0 by one; and, as the number template, the list's own
/// reference template where the list writes one
/// ([`ListOptions::ref_template`]). The item's counter value is therefore
/// recorded in the form a reference to it shows, and the list's rule writes
/// the label beside the item from the same record.
///
/// # The content the reference point carries
///
/// A reference to an item shows what is shown beside the item, so the point
/// carries the item's label as its content wherever the label is not a
/// number:
///
/// - for a tagged item, its tag, however it is written (`\item[Custom]`,
///   `\item[tag=Custom]`, `\item[tag={Custom}]`); such an item declares no
///   number source, so it is counted in no counter;
/// - for an untagged item of an unnumbered list, the list's own label where
///   its bracket writes one that can be used (`[-]`, `[label=*]`,
///   `[label={--}]`), and otherwise the label template its list kind
///   declares for the depth, by default the bullet `•` of an `itemize`. A
///   list's own label that holds a placeholder (`label=#{arabic}`, `[1.]`)
///   is an error on an unnumbered list ([`LabelNamesNumber`]); the item
///   shows the depth's label instead, and a reference to the item shows the
///   same;
/// - for any other item, nothing: its number is what a reference shows.
///
/// This content is not a node of the document that the point could refer to
/// (a bullet comes from the configuration, and `Custom` in `tag=Custom`
/// shares a run of characters with its key), so the point carries it as a
/// small tree of its own ([`ContentRef::Synthesized`]). The bullet's tree is
/// built once per configured template, when the registry is built, and
/// shared by every item that shows it. A tag or a list's own label is copied
/// into a new tree each time the points are computed, which the
/// [`refpoints`](crate::stdcore::features::refpoints) feature does once per
/// run.
///
/// # The scoped instances under which an item is counted
///
/// Each list is a scoped instance of its list kind's counter (see
/// [scoped instances](crate::model::aspects::NumberSource#scoped-instances)),
/// and a nested list's instance is numbered within the number of the item
/// that holds it. A tagged item has no number, so no list can be numbered
/// within it. An item is therefore counted under its own list and, among the
/// enclosing lists of the same list kind, under each one whose item holding
/// the nested list is untagged. A list held by a tagged item, or written
/// before the first `\item` of the enclosing list, is numbered within the
/// nearest untagged item of its list kind further out; where there is none,
/// it is an outermost instance. The nesting depth, which selects the counter
/// style, still counts every enclosing list of the list kind. In an
/// `enumerate`, `\item[T] \begin{enumerate}\item x\end{enumerate}` therefore
/// numbers `x` as `(i)` regardless of the items before `T`, and no nesting
/// joiner writes a number of the enclosing list.
impl Referenceable for ItemElement {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let (Some(list), Some(item)) = (enclosing_list(node), item_of(node)) else {
            return Vec::new();
        };
        let Some(value) = element_value::<ListElement>(&list) else { return Vec::new() };
        let (labels, malformed) = refs::point_labels(item.labels.iter());
        let options = list_options(&list);
        let depth = nesting_chain(&list).len();
        let number = (value.kind.numbered && item.tag.is_none()).then(|| NumberSource {
            counter: numbers::counter_name(value.name.as_str()),
            style: numbers::style_name(value.name.as_str(), depth, value.kind.depth_count()),
            instances: instance_chain(&list).iter().map(|list| list.id()).collect(),
            advance: vec![0],
            number_format: options.ref_template().map(String::from),
        });
        let content = match (&item.tag, &options.label) {
            (Some(tag), _) => self.copied(node, &tag.pieces),
            (None, Some(own)) if !value.kind.numbered && !holds_placeholder(&own.template) => {
                self.copied(node, &own.pieces)
            }
            (None, _) if !value.kind.numbered => self
                .labels
                .of_kind(&value.name, depth)
                .and_then(|label| label.fixed().ok().flatten())
                .map(ContentRef::Synthesized),
            _ => None,
        };
        vec![RefPoint {
            kind: RefKind::from(KIND),
            labels,
            malformed,
            computed: 0,
            anchor: AnchorPolicy::Auto { prefix: ANCHOR_PREFIX.to_string() },
            number,
            content,
            accepts: AcceptedRefTypes::these(LABEL_TYPES),
        }]
    }
}

impl ItemElement {
    /// `pieces`, a tag or a list's own label, copied into a tree of their own
    /// beside the `\item` node `item`, as the content of its reference point.
    ///
    /// A copy that fails (content nesting deeper than the recursion guard
    /// allows, which the parse has already refused) gives no content; an
    /// aspect reports nothing.
    // See decision rationale at [§flm-dd-dr:std-list-item-reference].
    fn copied(&self, item: &NodeRef<'_, Flm>, pieces: &[WrittenPiece<'_>]) -> Option<ContentRef> {
        let pieces: Vec<ValuePiece<'_, '_>> = pieces.iter().map(WrittenPiece::as_value_piece).collect();
        let guard = self.labels.guard();
        // A reference to the item shows the copy; the item shows the tag where
        // it stands.
        on_worker_thread(|| copy_pieces_into_value(&pieces, item, ValueContent::Shown, guard))
            .ok()?
            .ok()
            .map(ContentRef::Synthesized)
    }
}

/// The element value of the list kind `name`: a block element carrying the
/// list kind.
fn list_element(name: &Name, kind: &ListKind) -> Arc<ElementSpec> {
    let numbered = if kind.numbered { "numbered" } else { "unnumbered" };
    Arc::new(
        ElementSpec::new(LIST)
            .docs(format!("`{name}`: a list whose items are {numbered}."))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with_value(Arc::new(ListElement { name: name.clone(), kind: kind.clone() })),
    )
}

/// The element value of `\item`: a block element, so that it remains a
/// direct child of a list's body, between the blocks of the items; its
/// reference point reads the list kinds' compiled label templates `labels`.
fn item_element(labels: Arc<Labels>) -> Arc<ElementSpec> {
    // A block keeps every `\item` a direct child of the list's body through
    // the block-structure pass, which is where the item reader splits it.
    // See decision rationale at [§flm-dd-dr:std-list-reading].
    let value = Arc::new(ItemElement { labels });
    Arc::new(
        ElementSpec::new(ITEM)
            .docs("`\\item`: begins an item of the list in whose body it is written.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
            .with_value(value),
    )
}

/// The scoped package [`PACKAGE`], declared by `owner`, holding `\item` with
/// its optional argument and its `\label` fields; `labels` are the list
/// kinds' compiled label templates, which an item's reference point reads.
fn package(owner: &crate::model::names::FeatureName, labels: Arc<Labels>) -> ScopedPackageDef {
    let item = ElementDef::new(
        owner.clone(),
        [ITEM_COMMAND],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals)
                    .docs("The item's tag, `tag={…}`, or the tag whole where no `=` is written."),
                // The labels are the `\item`'s own tack-on fields, as a
                // heading's are: an inline field read from the item's
                // content would stand inside its first paragraph.
                // See decision rationale at [§flm-dd-dr:std-list-label-fields].
                refs::label_fields_arg(),
            ],
        },
        item_element(labels),
    )
    .docs("`\\item[tag]\\label{item:…}`: begins an item; the optional tag is its label.");
    ScopedPackageDef {
        owner: owner.clone(),
        name: Name::from(PACKAGE),
        elements: vec![item],
        docs: String::from(
            "The command recognized inside a list environment's body: `\\item`, which begins \
             an item.",
        ),
    }
}

/// The `lists` feature ([`FlmFeature`]). Under a [`ListsConfig`], it
/// registers one `flm.lists.list` element value per list kind, written as
/// the environment of the list kind's name; the scoped package holding
/// `\item`; the counters and counter styles of the numbered list kinds, as
/// peer contributions to the numbering feature; the build hook
/// ([`LateInit`](crate::core::registry::LateInit)) that compiles every list
/// kind's label templates and a numbered list kind's reference templates;
/// and the lowering rules [`RULE`], [`ITEM_RULE`] and
/// [`latex::LATEX_RULE`]. See the
/// [module documentation](self#what-the-feature-registers).
#[derive(Debug, Clone, Default)]
pub struct Lists {
    config: ListsConfig,
}

impl Lists {
    /// The feature under `config`, which is fixed for the life of the
    /// registry it is built into (see
    /// [configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
    pub fn new(config: ListsConfig) -> Lists {
        Lists { config }
    }

    /// The configuration this feature was constructed with.
    pub fn config(&self) -> &ListsConfig {
        &self.config
    }
}

impl FlmFeature for Lists {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Lists").docs(
            "The list environments as element values of `flm.lists.list`, `\\item` inside their \
             bodies, the label shown beside each item, and one reference point per item.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        self.config
            .validate()
            .map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
        // The `\label` field of an item is the references feature's, with that
        // feature's role on its argument; declared as a peer, so that a
        // registry without that feature still builds.
        b.peer_argument_role(refs::DEFINED_LABEL_ARG_ROLE)?;
        for (name, kind) in &self.config.environments {
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [name.as_str()],
                    SyntaxDef::Environment {
                        args: vec![ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals).docs(
                            "The list's own label template, `label=…`, or a short form such as \
                             `(i)`, and its own reference template, `ref=…`.",
                        )],
                        body: BodyDef::new().blocks().scoped(PACKAGE),
                    },
                    list_element(name, kind),
                )
                .docs(format!("`\\begin{{{name}}}[label=…, ref=…]…\\end{{{name}}}`: a list.")),
            )?;
        }
        let labels = Arc::new(Labels::new(self.config.clone()));
        b.scoped_package(package(&owner, Arc::clone(&labels)))?;
        numbers::declare(b, &self.config)?;
        // The counter table belongs to the numbering feature, which may be
        // absent: the rule then shows no numbers and says so.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the numbering feature's counter table, which the list rule reads for the number \
             of each item",
        )?;
        // The anchors table belongs to the references feature, which may be
        // absent too: the rule then writes no item's anchor.
        b.peer_table::<refs::Anchors>(
            refs::ANCHORS.name(),
            "the references feature's anchors table, which the list rule reads for the anchor \
             of each item",
        )?;
        b.late_init(Arc::new(CompileLabels { labels: Arc::clone(&labels) }))?;
        b.lowering_rule(
            LoweringRule::new(
                RULE,
                LIST,
                LoweringDecl::new().reads_opt(COUNTERS).reads_opt(refs::ANCHORS).produces(pres::LIST),
                Arc::new(ListRule::new(Arc::clone(&labels))),
                DefaultMark::Everywhere,
            )
            .docs(
                "a list lowers to `flm.pres.list` whose kind tokens are its structure, the \
                 environment's name and `labeled`, with each item's label, content and anchor",
            ),
        )?;
        b.lowering_rule(
            LoweringRule::new(
                ITEM_RULE,
                ITEM,
                LoweringDecl::new(),
                Arc::new(ItemRule),
                DefaultMark::Everywhere,
            )
            .docs("an `\\item` that begins no item lowers to nothing and is reported"),
        )?;
        b.digest_input(&self.config.digest_bytes())?;
        latex::register(b, labels)?;
        Ok(())
    }
}

/// The [`Lists`] feature under the default configuration (the list kinds
/// `itemize`, `enumerate` and `description`), ready to be listed in a
/// package entry.
///
/// Use `Arc::new(Lists::new(config))` to list the feature under a
/// [`ListsConfig`] of your own.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Lists::default())
}
