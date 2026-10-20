//! Safe labels: the keys that the recomposed LaTeX writes in place of the
//! document's own labels, numbered in document order by a stage of the
//! [`refs`](super) feature and recorded in the table [`SAFE_LABELS`].
//!
//! The `latex-recomposed` backend writes the document back as LaTeX and
//! leaves references to LaTeX to resolve. A safe label ([`SafeLabels`]) is the
//! key the recomposed LaTeX writes for an FLM label: `ref1`, `ref2`, … in the
//! order the document first mentions the labels, one count per document
//! regardless of the label's type. An FLM label such as `sec:intro` or
//! `eq:my equation` is not a safe LaTeX key (LaTeX itself accepts a colon or a
//! space, but not every package that reads label keys does), so the
//! recomposed LaTeX writes the safe label instead, and the table
//! [`SAFE_LABELS`] holds the correspondence in both directions, for a tool
//! that reads the recomposed LaTeX beside the document. The rendering guide
//! shows the result under [References and safe
//! labels](crate::guide::rendering#references-and-safe-labels).
//!
//! # The stage and what counts as a mention
//!
//! The stage [`STAGE`] (`flm.refs.safe-labels`) runs once for the document,
//! reads the reference points ([`REFPOINTS`]) and writes [`SAFE_LABELS`]
//! (`flm.labels.safe`). It walks every node of every frozen tree in the run's
//! document order ([`StageCx::walk`]) and allocates the next safe label at the
//! first mention of a label. A label is mentioned in two ways:
//!
//! - a node that declares reference points mentions the labels of those
//!   points (the labels its construct read from its `\label` fields);
//! - a node that declares reference sites ([`Referencing`], the sites the
//!   resolve stage resolves) mentions the target of each site, whether or not
//!   anything carries the target: `\ref`, `\hyperref`, the `math` feature's
//!   `\eqref` and the `defterm` feature's term link alike.
//!
//! Every reference is therefore written with a safe label, even one that
//! points at nothing, and no rule writes a target's own characters (which may
//! hold markup, as a term's do) into a LaTeX label key. A target of `\ref` or
//! `\hyperref` that is not spelled `type:label` declares no site; it is
//! mentioned too, keyed by its text (below). A later mention of the same
//! label finds its safe label already allocated, so a forward reference takes
//! the lower number: `\ref{sec:b}` written before `\section{B}\label{sec:b}`
//! makes `sec:b` `ref1`.
//!
//! The stage never looks at `\label` nodes. A label that no construct turned
//! into a reference point is data that the construct absorbed and did not use
//! (for example a user definition's own `\label` field, whose meaning is up to
//! that definition), and it gets no safe label, because nothing in the
//! recomposed LaTeX writes it.
//!
//! The stage allocates nothing for a label whose type the render option
//! [`safe_label_types`](super::RefsRecomposeOptions::safe_label_types)
//! marks as [`Raw`](super::SafeLabelMode::Raw). It checks nothing else: a
//! label whose type its construct refuses (for example `\label{eq:x}` on a
//! heading) is allocated a safe label and recomposed like any other, and the
//! label check ([`CHECK_LABELS`](super::CHECK_LABELS)) reports it. The label
//! check is a member of the [`flm.validated`](crate::stdcore::VALIDATED)
//! collection of capabilities, which every workflow of the standard package
//! demands (asks the plan to provide; see
//! [`PlanRequest`](crate::core::plan::PlanRequest)), so a recomposing render
//! of one of those workflows reports it.
//!
//! **A label not spelled `type:label`** (`\label{intro}`, with no colon) is
//! allocated a safe label too, keyed by its text and of no type. Such a text
//! defines no label, so the reference point carries it among its
//! [malformed](crate::model::aspects::RefPoint::malformed) texts; it is
//! written like any other label, so that the recomposed LaTeX compiles and a
//! reference to it resolves in LaTeX. The label scan reports the malformed
//! text ([`BadLabel`](super::BadLabel)). Since it has no type,
//! `safe_label_types` cannot select it.
//!
//! # The table
//!
//! The table is keyed by a *domain* and, inside it, by the label's key
//! ([`label_key`]): its `type:label` spelling, or its text where it has no
//! type. The only domain today is [`REF_DOMAIN`] (`ref`), the domain of
//! `\label` and `\ref`. The count is shared across domains, so a feature that
//! allocates keys of another sort (citation keys, for example) can join
//! the same count under a domain of its own without collisions.
//!
//! The recomposition rules of this feature ([`super::latex`]) and every rule
//! that writes a construct's labels with [`stage_label`](super::stage_label)
//! (those of the headings, math, theorems, lists, defterm, floats and xnotes
//! features) read the table. Nothing else reads a document's labels in a plan
//! that recomposes it, since nothing is resolved there.
//!
//! # The references of a synthesized value
//!
//! Content spliced at lowering may write references of its own: a use site of
//! a user definition expanded where the recomposed LaTeX is written, or a
//! template lowered for the style file. Such content is a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) (a small tree a
//! stage builds during a run), not part of the document, so the stage above
//! never saw it. [Value
//! catch-up](crate::guide::concepts_overview#value-catch-up) brings it up to
//! date by running the per-tree stages the lowering depends on over the value
//! alone, and the per-tree stage [`VALUE_STAGE`]
//! (`flm.refs.value-safe-labels`) is one of them. Over a synthesized value,
//! it gives every target that the value's reference nodes mention (the same
//! mentions as above: each site's target, and each target of `\ref` or
//! `\hyperref` not spelled `type:label`, keyed by its text) and that has no
//! safe label in the document's table a *value name*: `ref-v1`, `ref-v2`, …
//! in the order the value first mentions the targets ([`value_name`]). It
//! writes the value names, per reference node, into [`VALUE_SAFE_LABELS`]
//! (`flm.labels.safe.value`), held in the
//! catch-up store of that value; over a document tree it writes nothing. The
//! rules read a target's safe label from the document's table first and from
//! the value's table second ([`target_spelling`](super::target_spelling)), so
//! no reference in a synthesized value is written with its own characters
//! either.
//!
//! The value names are numbered per synthesized value, starting from one in
//! each: one stage computes them over one value, regardless of the thread the
//! value is lowered on, so they never depend on the order in which work is
//! scheduled. They cannot equal a document's safe labels, which hold no
//! hyphen because their domain, `ref`, holds none ([`value_name`]). No label
//! is ever written under a value name: a synthesized value declares no label
//! (a label is a fact about the document, which a value may not state), so a
//! value name designates a target that nothing in the document carries (one
//! the resolve stage reports as unresolved, or a text that is not a label),
//! and LaTeX reports the reference as undefined, as it does for a document's
//! reference to a label that nothing carries. The same value name in two
//! synthesized values may designate two different targets; both are
//! undefined. A target that the resolve stage finds in the external index but
//! that the document never mentions also gets a value name, and is undefined
//! in LaTeX, as a document's reference to it is.
//!
//! The value stage reads the resolutions of the synthesized value, so that
//! the resolve stage ([`RESOLVE`](super::RESOLVE)) runs over the value in the
//! same catch-up and reports its references.

// Why one counter across domains and why the names carry no type prefix:
// see design rationale at [§flm-dd-dr:rend-recomposed-safe-labels].

use std::collections::BTreeMap;

use techy::error::Diagnostic;
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::model::addr::NodeAddr;
use crate::model::aspects::{Label, Referencing};
use crate::model::element::element_of;
use crate::lang::{nowhere, FlmOrigin};

use super::elements::reference_targets;
use super::latex::{RefsRecomposeOptions, SafeLabelMode};
use super::stages::StageFailed;
use super::tables::REF_RESOLUTIONS;
use crate::stdcore::features::index::ELEMENT_INDEX;
use crate::stdcore::features::refpoints::{REFPOINTS, RefPointRecord, node_key};

/// The name of the safe-labels stage, `flm.refs.safe-labels`: it walks the
/// document, reading the reference points and the render options
/// [`RefsRecomposeOptions`], and writes
/// [`SAFE_LABELS`]. See the module documentation.
pub const STAGE: &str = "flm.refs.safe-labels";

/// The typed name of the safe-labels table, `flm.labels.safe`, a
/// [`SafeLabels`], which the stage [`STAGE`] writes.
pub const SAFE_LABELS: TableName<SafeLabels> = TableName::new("flm.labels.safe");

/// The domain of the labels that `\label` writes and `\ref` points at, `ref`:
/// the first key of [`SafeLabels`], and the beginning of the safe labels
/// allocated in it (`ref1`, `ref2`, …).
pub const REF_DOMAIN: &str = "ref";

/// The key under which a label is allocated a safe label: its `type:label`
/// spelling with the whitespace around each part trimmed (the form in which
/// [`Label`] is displayed) when `label` is a parsed label, and `text` as
/// written when it is not (`intro`, with no colon). Two spellings of one
/// label, `sec:intro` and `sec : intro`, therefore share one safe label.
///
/// The stages and the rules all build the key through this function, so that
/// they agree.
///
/// # Examples
///
/// ```
/// use flm_core::model::aspects::Label;
/// use flm_core::stdcore::features::refs::label_key;
///
/// let label = Label::parse("sec : intro");
/// assert_eq!(label_key("sec : intro", label.as_ref()), "sec:intro");
/// assert_eq!(label_key("intro", None), "intro");
/// ```
pub fn label_key(text: &str, label: Option<&Label>) -> String {
    match label {
        Some(label) => label.to_string(),
        None => String::from(text),
    }
}

/// The safe-labels table, `flm.labels.safe` ([`SAFE_LABELS`]): per domain,
/// from a label's key ([`label_key`]) to the safe label allocated for it, and
/// back.
///
/// A safe label is the key the recomposed LaTeX writes for an FLM label
/// (`ref1`, `ref2`, … in the order the document first mentions the labels),
/// so that any label text an author wrote is carried through LaTeX's
/// auxiliary file. The recomposed LaTeX writes it in each `\label` (inside
/// the [LaTeX hook](crate::stdcore::backends::latex::preamble) `\flmLabels`
/// when the backend's render option
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
/// is on)
/// and in each reference.
/// A *value name* (`ref-v1`, `ref-v2`, …; [`value_name`]) is the key written
/// for a target that a reference inside a synthesized value mentions and that
/// has no safe label in the document; it is numbered apart from the
/// document's safe labels so that the two sequences never collide. See the
/// module documentation.
///
/// Read the safe label of a key with [`safe_name`](SafeLabels::safe_name),
/// and the key of a safe label with [`label_of`](SafeLabels::label_of). A key
/// the table does not hold is one the stage allocated nothing for (a label of
/// a type written raw), which a rule writes as written.
#[derive(Debug, Clone, Default)]
pub struct SafeLabels {
    /// Domain → the label's key → the safe label allocated for it.
    pub by_label: BTreeMap<String, BTreeMap<String, String>>,
    /// Domain → the safe label → the label's key, the reverse of
    /// `by_label`.
    pub by_name: BTreeMap<String, BTreeMap<String, String>>,
    /// How many safe labels have been allocated across every domain: the
    /// next one's number is one more.
    pub allocated: u32,
}

impl SafeLabels {
    /// The safe label allocated for the key `key` in `domain`, or `None` when
    /// the stage allocated none for it. Build `key` with [`label_key`].
    pub fn safe_name(&self, domain: &str, key: &str) -> Option<&str> {
        self.by_label.get(domain)?.get(key).map(String::as_str)
    }

    /// The label key for which the safe label `name` was allocated in
    /// `domain`, or `None` for a safe label the table does not hold.
    pub fn label_of(&self, domain: &str, name: &str) -> Option<&str> {
        self.by_name.get(domain)?.get(name).map(String::as_str)
    }

    /// The safe label of the key `key` in `domain`, allocating the next one
    /// when the key has none yet: the domain followed by the number of safe
    /// labels allocated so far across every domain, plus one.
    pub fn allocate(&mut self, domain: &str, key: &str) -> &str {
        let names = self.by_label.entry(String::from(domain)).or_default();
        if !names.contains_key(key) {
            self.allocated += 1;
            let name = format!("{domain}{}", self.allocated);
            self.by_name
                .entry(String::from(domain))
                .or_default()
                .insert(name.clone(), String::from(key));
            names.insert(String::from(key), name);
        }
        names.get(key).map(String::as_str).unwrap_or_default()
    }
}

impl Table for SafeLabels {
    /// The entries of a table piece are added to this table's; a key that
    /// already has a safe label keeps it. The stage writes the whole table at
    /// once, so two table pieces never hold the labels of one document.
    fn absorb(&mut self, piece: SafeLabels) {
        for (domain, names) in piece.by_label {
            let mine = self.by_label.entry(domain).or_default();
            for (key, name) in names {
                mine.entry(key).or_insert(name);
            }
        }
        for (domain, labels) in piece.by_name {
            let mine = self.by_name.entry(domain).or_default();
            for (name, label) in labels {
                mine.entry(name).or_insert(label);
            }
        }
        self.allocated = self.allocated.max(piece.allocated);
    }

    /// Projects the table as `{<domain>: {labels: {<key>: <safe label>},
    /// names: {<safe label>: <key>}}}`.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.by_label
                .iter()
                .map(|(domain, names)| {
                    let labels = names
                        .iter()
                        .map(|(key, name)| (key.clone(), SerialValue::Str(name.clone())))
                        .collect();
                    let reverse = self
                        .by_name
                        .get(domain)
                        .into_iter()
                        .flatten()
                        .map(|(name, key)| (name.clone(), SerialValue::Str(key.clone())))
                        .collect();
                    (
                        domain.clone(),
                        SerialValue::Map(vec![
                            (String::from("labels"), SerialValue::Map(labels)),
                            (String::from("names"), SerialValue::Map(reverse)),
                        ]),
                    )
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.by_label.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

/// The safe-labels stage [`STAGE`] (`flm.refs.safe-labels`). See the module
/// documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct SafeLabelsStage;

impl Stage for SafeLabelsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(STAGE, Effect::Compute, Scope::Document).writes(SAFE_LABELS).docs(
            "For the recomposed LaTeX: allocates the safe label (`ref1`, `ref2`, …) of every \
             label a reference point carries and of every reference target, resolved or not, \
             in the order the document first mentions it, into `flm.labels.safe`.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(REFPOINTS).option::<RefsRecomposeOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let options = match cx.option::<RefsRecomposeOptions>() {
            Ok(options) => options,
            Err(error) => {
                out.diags.push(Diagnostic::error(
                    StageFailed { stage: String::from(STAGE), detail: error.to_string() },
                    nowhere(FlmOrigin::none()),
                ));
                return;
            }
        };
        let points = match cx.get(REFPOINTS) {
            Ok(Some(points)) => points,
            // The read is declared and the table is written by a scheduled
            // stage, so this cannot happen; saying so is better than writing
            // an empty table for a document full of labels.
            Ok(None) => {
                out.diags.push(Diagnostic::error(
                    StageFailed {
                        stage: String::from(STAGE),
                        detail: String::from("the reference points were not computed"),
                    },
                    nowhere(FlmOrigin::none()),
                ));
                return;
            }
            Err(error) => {
                out.diags.push(Diagnostic::error(
                    StageFailed { stage: String::from(STAGE), detail: error.to_string() },
                    nowhere(FlmOrigin::none()),
                ));
                return;
            }
        };
        // The points of one node, in the order the node declares them: the
        // table holds one node's points in that order, so appending keeps it.
        let mut by_node: BTreeMap<NodeAddr, Vec<&RefPointRecord>> = BTreeMap::new();
        for record in points.records() {
            by_node.entry(record.id.addr).or_default().push(record);
        }
        let mut table = SafeLabels::default();
        // A label with no type is allocated a name like any other, keyed by
        // its text; `safe_label_types` names types, so it cannot name it.
        let raw = |label: Option<&Label>| {
            label.is_some_and(|label| {
                options.safe_label_types.get(&label.ref_type) == Some(&SafeLabelMode::Raw)
            })
        };
        for (addr, node) in cx.walk() {
            // What this node mentions: everything written as a label for every
            // point it declares, in the order the document wrote it — a text
            // that is no label among them, keyed by its text and written like
            // any other, so that the recomposed LaTeX still pairs it with the
            // references that name it.
            let mentions = by_node
                .get(&addr)
                .into_iter()
                .flatten()
                .flat_map(|record| record.mentions());
            for (text, label) in mentions {
                if !raw(label) {
                    table.allocate(REF_DOMAIN, &label_key(&text, label));
                }
            }
            // A reference mentions the target of every site it declares — the
            // sites the resolver resolves, read through the same aspect, so
            // that every referencing element of every feature (`\ref`,
            // `\hyperref`, `\eqref`, a term link) gets a key for its target
            // alike, resolved or not, and no rule has to write a target the
            // table does not hold. A `\ref` may name several targets at once,
            // each of which is written by a key of its own.
            let sites = element_of(&node)
                .aspect::<dyn Referencing>()
                .map(|referencing| referencing.sites(&node))
                .unwrap_or_default();
            for site in sites {
                if !raw(Some(&site.target)) {
                    table.allocate(REF_DOMAIN, &label_key("", Some(&site.target)));
                }
            }
            // A target of `\ref` or `\hyperref` that is not spelled
            // `type:label` declares no site, and is named like a label with no
            // type, keyed by its text, so that it is written as a key and never
            // as the characters the document wrote.
            let unspelled = reference_targets(&node).into_iter().flatten();
            for target in unspelled.filter(|target| target.label.is_none()) {
                table.allocate(REF_DOMAIN, &label_key(&target.text, None));
            }
        }
        match out.table(SAFE_LABELS) {
            Ok(written) => *written = table,
            Err(error) => out.diags.push(Diagnostic::error(
                StageFailed { stage: String::from(STAGE), detail: error.to_string() },
                nowhere(FlmOrigin::none()),
            )),
        }
    }
}

// --- the references a value writes ------------------------------------------------------

/// The name of the value stage, `flm.refs.value-safe-labels`: over a
/// synthesized value spliced at lowering, it reads [`SAFE_LABELS`], the
/// value's element index and resolutions and the render options, and writes
/// [`VALUE_SAFE_LABELS`]. See the module documentation, *The references of a
/// synthesized value*.
pub const VALUE_STAGE: &str = "flm.refs.value-safe-labels";

/// The typed name of the value names table, `flm.labels.safe.value`, a
/// [`ValueSafeLabels`], which the stage [`VALUE_STAGE`] writes.
pub const VALUE_SAFE_LABELS: TableName<ValueSafeLabels> = TableName::new("flm.labels.safe.value");

/// The value name numbered `number` in `domain`: the domain, a hyphen, `v`
/// and the number, as in `ref-v1`. A document's safe labels
/// ([`SafeLabels::allocate`]) are the domain followed by the number alone, so
/// for a domain without a hyphen (such as [`REF_DOMAIN`], the only domain the
/// library allocates in) a document's safe label holds no hyphen, and the two
/// never coincide. See the module documentation, *The references of a
/// synthesized value*.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::{REF_DOMAIN, value_name};
///
/// assert_eq!(value_name(REF_DOMAIN, 1), "ref-v1");
/// ```
pub fn value_name(domain: &str, number: u32) -> String {
    format!("{domain}-v{number}")
}

/// The value names table, `flm.labels.safe.value` ([`VALUE_SAFE_LABELS`]):
/// for each reference node of a synthesized value, from the key of a target
/// it mentions ([`label_key`]) to the value name that target is written
/// under, for the targets that have no safe label in the document's table
/// ([`SafeLabels`]). Empty for every document tree; see the module
/// documentation, *The references of a synthesized value*.
///
/// It is held in the catch-up store of one synthesized value and read only
/// at the addresses of that value's nodes, so its node addresses never reach
/// the run's own store.
#[derive(Debug, Clone, Default)]
pub struct ValueSafeLabels {
    /// Reference node → the target's key → the value name written for it, in
    /// the domain [`REF_DOMAIN`].
    pub by_node: BTreeMap<NodeAddr, BTreeMap<String, String>>,
}

impl ValueSafeLabels {
    /// The value name written for the target of key `key` that the node at
    /// `at` mentions, or `None` where the table holds none. Build `key` with
    /// [`label_key`].
    pub fn safe_name(&self, at: NodeAddr, key: &str) -> Option<&str> {
        self.by_node.get(&at)?.get(key).map(String::as_str)
    }
}

impl Table for ValueSafeLabels {
    fn absorb(&mut self, piece: ValueSafeLabels) {
        for (addr, names) in piece.by_node {
            self.by_node.entry(addr).or_default().extend(names);
        }
    }

    /// Projects the table as `{<node>: {<key>: <value name>}}`.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.by_node
                .iter()
                .map(|(addr, names)| {
                    let names = names
                        .iter()
                        .map(|(key, name)| (key.clone(), SerialValue::Str(name.clone())))
                        .collect();
                    (node_key(cx, *addr), SerialValue::Map(names))
                })
                .collect(),
        )
    }

    /// Keyed by node address: the names under which one synthesized value's
    /// references are written are not a fact about the document.
    fn piece_class(&self) -> PieceClass {
        if self.by_node.is_empty() { PieceClass::Empty } else { PieceClass::NodeKeyed }
    }
}

/// The value stage [`VALUE_STAGE`] (`flm.refs.value-safe-labels`). See the
/// module documentation, *The references of a synthesized value*.
#[derive(Debug, Default, Clone, Copy)]
pub struct ValueSafeLabelsStage;

impl Stage for ValueSafeLabelsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(VALUE_STAGE, Effect::Compute, Scope::PerTree)
            .writes(VALUE_SAFE_LABELS)
            .docs(
                "For the recomposed LaTeX, over a synthesized value spliced at lowering: gives \
                 every target its references mention that has no safe label in \
                 `flm.labels.safe` a value name, `ref-v1`, `ref-v2`, … in the order the value \
                 first mentions it, into `flm.labels.safe.value`; writes nothing over a \
                 document tree.",
            )
    }

    /// Reads the document's safe labels, the element index of the tree, the
    /// render options for the label types written raw, and the resolutions:
    /// the last so that the resolve stage runs over the synthesized value
    /// before this stage, in the same catch-up, and reports the value's
    /// references.
    // Why the resolutions are read without being used: see decision rationale
    // at [§flm-dd-dr:std-refs-value-references].
    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(SAFE_LABELS)
            .reads(ELEMENT_INDEX)
            .reads(REF_RESOLUTIONS)
            .option::<RefsRecomposeOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (Some(id), Some(tree)) = (cx.tree_id(), cx.my_tree()) else { return };
        if !id.is_value() {
            return;
        }
        // A failure here is the plan's, and reported over the document by
        // the document-scope stage reading the same tables.
        let (Ok(Some(document)), Ok(Some(index)), Ok(options)) =
            (cx.get(SAFE_LABELS), cx.get(ELEMENT_INDEX), cx.option::<RefsRecomposeOptions>())
        else {
            return;
        };
        let raw = |label: &Label| {
            options.safe_label_types.get(&label.ref_type) == Some(&SafeLabelMode::Raw)
        };
        let mut named: BTreeMap<String, String> = BTreeMap::new();
        let mut table = ValueSafeLabels::default();
        for (addr, node, referencing) in
            index.nodes_with::<dyn Referencing>(cx.registry(), id, &tree)
        {
            // The same mentions as the document's stage reads: each site's
            // target, then each target not spelled `type:label`, by its text.
            let sites = referencing.sites(&node).into_iter().map(|site| site.target);
            let keys = sites
                .filter(|target| !raw(target))
                .map(|target| label_key("", Some(&target)))
                .chain(
                    reference_targets(&node)
                        .into_iter()
                        .flatten()
                        .filter(|target| target.label.is_none())
                        .map(|target| label_key(&target.text, None)),
                );
            for key in keys {
                if document.safe_name(REF_DOMAIN, &key).is_some() {
                    continue;
                }
                let next = named.len() + 1;
                let name = named
                    .entry(key.clone())
                    .or_insert_with(|| value_name(REF_DOMAIN, u32::try_from(next).unwrap_or(u32::MAX)))
                    .clone();
                table.by_node.entry(addr).or_default().insert(key, name);
            }
        }
        if let Ok(written) = out.table(VALUE_SAFE_LABELS) {
            *written = table;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_allocated_once_per_label_across_domains() {
        let mut table = SafeLabels::default();
        let a = Label::new("sec", "a").to_string();
        let b = Label::new("eq", "b").to_string();
        assert_eq!(table.allocate(REF_DOMAIN, &a), "ref1");
        assert_eq!(table.allocate("cite", &b), "cite2");
        assert_eq!(table.allocate(REF_DOMAIN, &a), "ref1");
        assert_eq!(table.allocate(REF_DOMAIN, &b), "ref3");
        assert_eq!(table.safe_name(REF_DOMAIN, &b), Some("ref3"));
        assert_eq!(table.safe_name("cite", &a), None);
        assert_eq!(table.label_of("cite", "cite2"), Some("eq:b"));
        assert_eq!(table.allocated, 3);
        assert_eq!(table.piece_class(), PieceClass::DocumentFact);
        assert_eq!(SafeLabels::default().piece_class(), PieceClass::Empty);
    }

    #[test]
    fn a_label_with_no_type_is_keyed_by_its_text_and_named_like_any_other() {
        assert_eq!(label_key("sec : intro", Label::parse("sec : intro").as_ref()), "sec:intro");
        assert_eq!(label_key("intro", None), "intro");
        let mut table = SafeLabels::default();
        assert_eq!(table.allocate(REF_DOMAIN, &label_key("sec:a", Label::parse("sec:a").as_ref())), "ref1");
        assert_eq!(table.allocate(REF_DOMAIN, &label_key("a_b", None)), "ref2");
        assert_eq!(table.safe_name(REF_DOMAIN, "a_b"), Some("ref2"));
        assert_eq!(table.label_of(REF_DOMAIN, "ref2"), Some("a_b"));
    }

    #[test]
    fn a_value_name_holds_a_hyphen_no_document_name_holds() {
        assert_eq!(value_name(REF_DOMAIN, 3), "ref-v3");
        let mut table = SafeLabels::default();
        assert!(!table.allocate(REF_DOMAIN, "sec:a").contains('-'));
        assert_eq!(ValueSafeLabels::default().piece_class(), PieceClass::Empty);
    }

    #[test]
    fn the_value_stage_is_per_tree_and_writes_its_table() {
        let decl = ValueSafeLabelsStage.decl();
        assert_eq!(decl.name, VALUE_STAGE);
        assert_eq!(decl.scope, Scope::PerTree);
        assert_eq!(decl.writes.len(), 1);
    }

    #[test]
    fn the_stage_is_of_document_scope_and_writes_the_table() {
        let decl = SafeLabelsStage.decl();
        assert_eq!(decl.name, STAGE);
        assert_eq!(decl.scope, Scope::Document);
        assert_eq!(decl.effect, Effect::Compute);
        assert_eq!(decl.writes.len(), 1);
    }
}
