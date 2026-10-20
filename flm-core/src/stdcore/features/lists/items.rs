//! Reading a list from the document tree: its items, split at the `\item`
//! nodes of its body, and the enclosing lists of the same list kind.
//!
//! [`list_items`] is the only reader of a list's body: every lowering rule of
//! a list stages the items it returns, and the reference point of every
//! `\item` is computed from the same reading of that `\item` ([`item_of`]),
//! so the rules and the reference points always agree on what an item is.
//! The functions here report nothing and never change the tree.

use techy::core::node::{NodeKind, NodeRef};

use crate::lang::Flm;
use crate::lang::structural;
use crate::model::element::element_value;
use crate::stdcore::features::refs::{self, LabelUse};

use super::syntax::{item_options, list_options, ItemTag, OwnLabel};
use super::{ListElement, ITEM};

/// One item of a list, as [`list_items`] reads it: its `\item`, its tag, its
/// labels and its content.
#[derive(Debug, Clone)]
pub struct ListItem<'t> {
    /// The `\item` node that begins the item.
    pub node: NodeRef<'t, Flm>,
    /// The tag the `\item` writes (`\item[My tag]`, `\item[tag={My tag}]`),
    /// which is the item's label when there is one.
    pub tag: Option<ItemTag<'t>>,
    /// The `\label` fields written after the `\item`, in order, each as
    /// written together with its parsed form when the text is spelled
    /// `type:label`.
    pub labels: Vec<LabelUse>,
    /// The item's content, in order: every node of the body after the
    /// `\item`, up to the next `\item` or the end of the body. Block content:
    /// paragraphs, nested lists, displayed formulas. Empty for an empty item.
    pub content: Vec<NodeRef<'t, Flm>>,
}

/// A list's body as [`list_items`] reads it, with the label templates that
/// apply to its items.
#[derive(Debug, Clone)]
pub struct ListBody<'t> {
    /// The list's nesting depth: the number of lists of the same list kind
    /// that contain it, itself included; 1 for a list inside no list of its
    /// list kind. It selects the list kind's label template and, for a
    /// numbered list kind, the counter style.
    pub depth: usize,
    /// The list's own label template, where its optional argument writes one
    /// that can be read ([`list_options`]). Not yet compiled.
    pub own_label: Option<OwnLabel<'t>>,
    /// The list's own reference template, where its optional argument writes
    /// one: its `ref=`, else the one its own label gives
    /// ([`ListOptions::ref_template`](super::ListOptions::ref_template)).
    /// Only a numbered list uses it.
    pub own_ref: Option<String>,
    /// The label template the list's list kind declares for
    /// [`depth`](Self::depth); `None` for a list kind that declares none, such
    /// as `description`.
    pub kind_label: Option<String>,
    /// The content written before the first `\item`, all of it. Empty unless
    /// that content holds something other than whitespace and comments,
    /// which is an error
    /// ([`ContentBeforeItem`](super::ContentBeforeItem)).
    pub before: Vec<NodeRef<'t, Flm>>,
    /// The items, in order.
    pub items: Vec<ListItem<'t>>,
}

/// Whether `node` is an `\item`.
pub fn is_item(node: &NodeRef<'_, Flm>) -> bool {
    node.ext().element.name.as_str() == ITEM
}

/// Whether `node`, a child of a list's body, holds nothing but layout:
/// whitespace, a comment, or a blank line.
fn is_blank(node: &NodeRef<'_, Flm>) -> bool {
    if node.ext().element.name.as_str() == structural::PARBREAK {
        return true;
    }
    match node.kind() {
        NodeKind::Comment(_) => true,
        _ => node.chars().is_some_and(|text| text.trim().is_empty()),
    }
}

/// The list in which the `\item` node `node` begins an item: the list
/// environment whose body holds `node` as a direct child. `None` for an
/// `\item` located anywhere else, such as in a group or an argument inside a
/// list, which begins no item.
pub fn enclosing_list<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeRef<'t, Flm>> {
    let region = node.parent()?;
    let list = region.parent()?;
    element_value::<ListElement>(&list)?;
    // The node stands in the body exactly when it shares the body's region;
    // an argument's content stands in a region of its own.
    let first = list.body()?.first()?;
    (first.parent()?.id() == region.id()).then_some(list)
}

/// The lists of the same list kind as `list` that contain `list`, `list`
/// included, outermost first: its *nesting chain*. Their number is `list`'s
/// nesting depth; for a node that is not a list, the chain is empty. For a
/// numbered list kind, the scoped instances under which an item of `list` is
/// counted are taken from this chain: `list` itself, and every list of the
/// chain whose item holding the next list is untagged (see
/// [`ItemElement`](super::ItemElement)).
///
/// Lists of another list kind are skipped: an `enumerate` inside an `itemize`
/// inside an `enumerate` is at depth 2, as in LaTeX. The walk goes up the
/// tree once, one step per enclosing node, and stays in `list`'s tree: a list
/// in another tree placed inside an item begins a chain of its own.
pub fn nesting_chain<'t>(list: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    let Some(value) = element_value::<ListElement>(list) else { return Vec::new() };
    let mut chain: Vec<NodeRef<'t, Flm>> = std::iter::successors(Some(*list), |node| node.parent())
        .filter(|node| {
            element_value::<ListElement>(node).is_some_and(|other| other.name == value.name)
        })
        .collect();
    chain.reverse();
    chain
}

/// The scoped instances under which an item of `list` is counted, outermost
/// first: `list` itself, and each enclosing list of the same list kind whose
/// item holding `list` is untagged. [`ItemElement`](super::ItemElement)
/// states the rule and the reason for it.
///
/// The walk goes up the tree once, as [`nesting_chain`]'s does. At each
/// enclosing list of the list kind, the item that holds the path down to
/// `list` is the nearest `\item` before that path in the list's body, found
/// by stepping back over the nodes of that item's content.
pub(super) fn instance_chain<'t>(list: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    let Some(value) = element_value::<ListElement>(list) else { return Vec::new() };
    let mut chain = vec![*list];
    // `child` stands in `region`, which stands in `node`.
    let (mut child, mut region) = match list.parent() {
        Some(region) => (*list, region),
        None => return chain,
    };
    while let Some(node) = region.parent() {
        let same_kind =
            element_value::<ListElement>(&node).is_some_and(|other| other.name == value.name);
        if same_kind
            && holding_item(&node, &region, &child)
                .is_some_and(|item| item_options(&item).tag.is_none())
        {
            chain.push(node);
        }
        child = region;
        region = node;
    }
    chain.reverse();
    chain
}

/// The `\item` of the list `list` whose content holds `child`, a node in
/// `region`, a region of `list`: the nearest `\item` before `child` in
/// `list`'s body. `None` where `region` is not the body (`child` is in an
/// argument), or where no `\item` precedes `child`, which is then content
/// before the first item.
fn holding_item<'t>(
    list: &NodeRef<'t, Flm>,
    region: &NodeRef<'t, Flm>,
    child: &NodeRef<'t, Flm>,
) -> Option<NodeRef<'t, Flm>> {
    if list.body()?.first()?.parent()?.id() != region.id() {
        return None;
    }
    let siblings = region.children();
    (0..child.index_in_parent()?).rev().filter_map(|index| siblings.get(index)).find(is_item)
}

/// The `\item` node `node` read on its own: its tag and its labels, with no
/// content. `None` for a node that is not an `\item`.
///
/// [`list_items`] reads each `\item` with this function, and the item's
/// reference point ([`ItemElement`](super::ItemElement)) is computed from
/// it.
pub fn item_of<'t>(node: &NodeRef<'t, Flm>) -> Option<ListItem<'t>> {
    if !is_item(node) {
        return None;
    }
    Some(ListItem {
        node: *node,
        tag: item_options(node).tag,
        labels: refs::label_uses(node),
        content: Vec::new(),
    })
}

/// The items of the list environment `node`, split at the `\item` nodes of
/// its body, with the label templates that apply to them.
///
/// Every direct child of the body after an `\item`, up to the next `\item`
/// or the end of the body, is that item's content. The content before the
/// first `\item` is skipped when it is only whitespace, blank lines and
/// comments, and is returned whole as [`before`](ListBody::before)
/// otherwise. For a node that is not a list, the result has no items and
/// depth 0.
///
/// Every rule that lowers a list calls this function, and nothing stores its
/// result.
pub fn list_items<'t>(node: &NodeRef<'t, Flm>) -> ListBody<'t> {
    let depth = nesting_chain(node).len();
    let kind_label = element_value::<ListElement>(node)
        .and_then(|value| value.kind.label_at(depth))
        .map(String::from);
    let mut body = ListBody {
        depth,
        own_label: None,
        own_ref: None,
        kind_label,
        before: Vec::new(),
        items: Vec::new(),
    };
    if depth == 0 {
        return body;
    }
    let options = list_options(node);
    body.own_ref = options.ref_template().map(String::from);
    body.own_label = options.label;
    let Some(children) = node.body() else { return body };
    let mut before = Vec::new();
    for child in children.iter() {
        if let Some(item) = item_of(&child) {
            body.items.push(item);
        } else if let Some(item) = body.items.last_mut() {
            item.content.push(child);
        } else {
            before.push(child);
        }
    }
    if !before.iter().all(is_blank) {
        body.before = before;
    }
    body
}
