//! The document model of `flm_core::model::doc`: fragments placed as trees, the
//! resolution map from node addresses to nodes, and the walk over a document.

mod common;

use std::sync::Arc;

use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::model::doc::{DocContent, Fragment, PlacementError, WalkStep};
use flm_core::lang::BuiltLang;

use common::{lang, parse};

fn fragment(lang: &BuiltLang, text: &str) -> Arc<Fragment> {
    Arc::new(parse(lang, text))
}

/// A compact rendering of a walk: `+n` enters tree `n`, `-n` leaves it, `n:c`
/// is a chars node of tree `n`, `n:l` a list node.
fn trace(doc: &DocContent) -> Vec<String> {
    doc.walk()
        .map(|step| match step {
            WalkStep::EnterTree(tree) => format!("+{}", tree.index()),
            WalkStep::ExitTree(tree) => format!("-{}", tree.index()),
            WalkStep::Node(addr, node) => {
                format!("{}:{}", addr.tree.index(), if node.is_chars() { "c" } else { "l" })
            }
        })
        .collect()
}

#[test]
fn placing_a_fragment_appends_a_tree_to_the_traversal() {
    let lang = lang(Vec::new());
    let mut doc = DocContent::new();
    let id = doc.place(fragment(&lang, "a"));
    assert_eq!(id, TreeId::tree(0));
    assert_eq!(doc.traversal(), [id]);
    assert!(Arc::ptr_eq(doc.fragment(id).unwrap(), doc.fragment(id).unwrap()));
}

#[test]
fn a_fragment_placed_twice_yields_two_trees() {
    let lang = lang(Vec::new());
    let shared = fragment(&lang, "a");
    let mut doc = DocContent::new();
    let first = doc.place(Arc::clone(&shared));
    let second = doc.place(Arc::clone(&shared));
    assert_ne!(first, second);
    assert_eq!(doc.walk_trees().collect::<Vec<_>>(), [first, second]);
}

#[test]
fn the_walk_visits_listed_trees_in_list_order_with_their_nodes() {
    let lang = lang(Vec::new());
    let mut doc = DocContent::new();
    doc.place(fragment(&lang, "a"));
    doc.place(fragment(&lang, "b"));
    assert_eq!(trace(&doc), ["+0", "0:l", "0:c", "-0", "+1", "1:l", "1:c", "-1"]);
}

#[test]
fn the_walk_enters_a_placed_tree_right_after_its_placing_node() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "ab");
    let inner = fragment(&lang, "c");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let b = doc.add_tree(inner);
    let site = outer.tree().root().child(0).unwrap().id();
    doc.resolve(NodeAddr::new(a, site), b).unwrap();
    assert_eq!(trace(&doc), ["+0", "0:l", "0:c", "+1", "1:l", "1:c", "-1", "-0"]);
}

#[test]
fn a_tree_reached_through_a_placement_is_not_walked_again_from_the_list() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "ab");
    let inner = fragment(&lang, "c");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let b = doc.place(inner);
    let site = outer.tree().root().child(0).unwrap().id();
    doc.resolve(NodeAddr::new(a, site), b).unwrap();
    assert_eq!(trace(&doc), ["+0", "0:l", "0:c", "+1", "1:l", "1:c", "-1", "-0"]);
}

#[test]
fn a_tree_neither_listed_nor_placed_is_not_walked() {
    let lang = lang(Vec::new());
    let mut doc = DocContent::new();
    let listed = doc.place(fragment(&lang, "a"));
    doc.add_tree(fragment(&lang, "b"));
    assert_eq!(doc.walk_trees().collect::<Vec<_>>(), [listed]);
    assert_eq!(doc.len(), 2);
}

#[test]
fn a_tree_placed_at_two_sites_is_a_typed_error_naming_both() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "ab");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let b = doc.add_tree(fragment(&lang, "c"));
    let root = outer.tree().root();
    let first = NodeAddr::new(a, root.id());
    let second = NodeAddr::new(a, root.child(0).unwrap().id());
    doc.resolve(first, b).unwrap();
    assert_eq!(
        doc.resolve(second, b),
        Err(PlacementError::AlreadyPlaced { target: b, first, second })
    );
}

#[test]
fn a_site_placing_twice_is_a_typed_error() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "a");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let b = doc.add_tree(fragment(&lang, "b"));
    let c = doc.add_tree(fragment(&lang, "c"));
    let site = NodeAddr::new(a, outer.tree().root().id());
    doc.resolve(site, b).unwrap();
    assert_eq!(doc.resolve(site, c), Err(PlacementError::SiteTaken { site, target: b }));
}

#[test]
fn an_unknown_tree_cannot_be_placed() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "a");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let site = NodeAddr::new(a, outer.tree().root().id());
    let missing = TreeId::tree(7);
    assert_eq!(doc.resolve(site, missing), Err(PlacementError::UnknownTree(missing)));
    let value = TreeId::value(0);
    assert_eq!(doc.resolve(site, value), Err(PlacementError::UnknownTree(value)));
}

#[test]
fn a_tree_cannot_place_itself() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "a");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let site = NodeAddr::new(a, outer.tree().root().id());
    assert_eq!(doc.resolve(site, a), Err(PlacementError::SelfPlacement(a)));
}

#[test]
fn the_resolution_map_answers_both_directions() {
    let lang = lang(Vec::new());
    let outer = fragment(&lang, "a");
    let mut doc = DocContent::new();
    let a = doc.place(Arc::clone(&outer));
    let b = doc.add_tree(fragment(&lang, "b"));
    let site = NodeAddr::new(a, outer.tree().root().id());
    doc.resolve(site, b).unwrap();
    assert_eq!(doc.placement_of(site), Some(b));
    assert_eq!(doc.placed_at(b), Some(site));
    assert_eq!(doc.placed_at(a), None);
}

#[test]
fn walk_nodes_addresses_every_node_of_every_walked_tree() {
    let lang = lang(Vec::new());
    let mut doc = DocContent::new();
    let a = doc.place(fragment(&lang, "ab"));
    let b = doc.place(fragment(&lang, "c"));
    let addrs: Vec<NodeAddr> = doc.walk_nodes().map(|(addr, _)| addr).collect();
    let trees: Vec<TreeId> = addrs.iter().map(|addr| addr.tree).collect();
    assert_eq!(trees, [a, a, b, b]);
}
