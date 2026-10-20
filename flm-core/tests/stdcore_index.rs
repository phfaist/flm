//! The `index` feature (`flm_core::stdcore::features::index`): the element
//! index lists every interned node of a frozen tree in document order, the
//! aspect-driven helper filters it through the registry's bitsets, the value
//! helper downcasts the element's per-value data, the document-wide helper
//! covers every tree in the run's order, and the table projects its node
//! references per tree.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::model::names::ElementName;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Run, RunOptions};
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::index::{self, ELEMENT_INDEX, ElementIndex, walk_nodes_with};
use techy::serialize::SerialValue;

use common::pipeline::{self, probe, record, recorded};
use common::stdcore::projection;

/// A test-only aspect.
trait Marked: Send + Sync {
    fn mark(&self) -> &str;
}

/// A test-only element value, also implementing the aspect.
#[derive(Debug)]
struct Tag(&'static str);

impl Marked for Tag {
    fn mark(&self) -> &str {
        self.0
    }
}

/// `\emph{…}` as the element `t.emph`, marked and valued with `Tag(mark)`.
fn emph(mark: &'static str) -> ElementDef {
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.emph"))
            .with::<dyn Marked>(Arc::new(Tag(mark)))
            .with_value(Arc::new(Tag(mark))),
    );
    let args = vec![ArgDef::new(ArgStdKind::Mandatory)];
    ElementDef::new("t", ["emph"], SyntaxDef::Macro { args }, element)
}

/// A registry over the `index` and `blocks` features and a toy feature
/// declaring `elements`.
fn registry(elements: Vec<ElementDef>) -> Arc<FlmRegistry> {
    let toy = pipeline::feature("t", move |b| {
        for element in &elements {
            b.element(element.clone())?;
        }
        Ok(())
    });
    pipeline::registry(vec![index::feature(), Arc::new(Blocks::default()), toy])
}

/// A run over one document of the given texts demanding the index over
/// structured trees.
fn indexed(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    let doc = pipeline::doc(registry, texts);
    let plan = pipeline::plan(registry, &["flm.element-index", "flm.blocks.structured"]);
    pipeline::run(&doc, &plan, &RunOptions::sequential())
}

fn index(run: &Run) -> &ElementIndex {
    run.table(ELEMENT_INDEX).expect("declared").expect("written")
}

#[test]
fn the_index_lists_every_node_of_a_tree_in_document_order() {
    let run = indexed(&registry(vec![emph("m")]), &["a \\emph{b}\n\nc"]);
    let tree = run.frozen_tree(TreeId::tree(0)).expect("frozen");
    let root = tree.root();
    let document_order: Vec<_> = std::iter::once(root.id())
        .chain(root.descendants().map(|node| node.id()))
        .collect();
    let indexed: Vec<_> = index(&run).entries(TreeId::tree(0)).iter().map(|(id, _)| *id).collect();
    assert_eq!(indexed, document_order);
    let names: Vec<String> = indexed
        .iter()
        .map(|id| element_of(&tree.get(*id).unwrap()).name.to_string())
        .collect();
    assert_eq!(names[0], "flm.container");
    assert_eq!(names[1], "flm.paragraph");
    assert!(names.contains(&"t.emph".to_string()));
}

#[test]
fn nodes_with_yields_the_aspect_carrying_nodes_in_document_order() {
    let run = indexed(&registry(vec![emph("m")]), &["\\emph{a} and \\emph{b}"]);
    let tree = run.frozen_tree(TreeId::tree(0)).expect("frozen");
    let found: Vec<(String, &str)> = index(&run)
        .nodes_with::<dyn Marked>(run.plan().registry(), TreeId::tree(0), tree)
        .map(|(addr, node, marked)| {
            assert_eq!(addr.tree, TreeId::tree(0));
            assert_eq!(addr.node, node.id());
            (node.argument_content_nodes(0).unwrap().first().unwrap().chars().unwrap().into(),
             marked.mark())
        })
        .collect();
    assert_eq!(found, [("a".to_string(), "m"), ("b".to_string(), "m")]);
}

#[test]
fn nodes_with_an_aspect_nobody_carries_yields_nothing() {
    trait Other: Send + Sync {}
    let run = indexed(&registry(vec![emph("m")]), &["\\emph{a}"]);
    let tree = run.frozen_tree(TreeId::tree(0)).expect("frozen");
    let registry = run.plan().registry();
    let count = index(&run).nodes_with::<dyn Other>(registry, TreeId::tree(0), tree).count();
    assert_eq!(count, 0);
}

#[test]
fn values_of_downcasts_the_elements_value() {
    let run = indexed(&registry(vec![emph("tagged")]), &["\\emph{a}"]);
    let tree = run.frozen_tree(TreeId::tree(0)).expect("frozen");
    let values: Vec<&str> =
        index(&run).values_of::<Tag>(TreeId::tree(0), tree).map(|(_, _, tag)| tag.0).collect();
    assert_eq!(values, ["tagged"]);
}

#[test]
fn walk_nodes_with_covers_every_tree_in_the_runs_order() {
    let seen = probe();
    let observer = Arc::clone(&seen);
    let walk = pipeline::stage(
        StageDecl::new("t.walk", Effect::Compute, Scope::Document).provides("t.walked"),
        Needs::new().reads(ELEMENT_INDEX),
        move |cx, _out| {
            let index = cx.get(ELEMENT_INDEX).unwrap().expect("provided");
            for (addr, _, marked) in walk_nodes_with::<dyn Marked>(cx, index) {
                record(&observer, format!("{} {}", addr.tree, marked.mark()));
            }
        },
    );
    let element = emph("m");
    let toy = pipeline::feature("t", move |b| {
        b.element(element.clone())?;
        b.capability(flm_core::core::registry::CapabilityDef::new("t.walked"))?;
        b.stage(Arc::clone(&walk))
    });
    let registry = pipeline::registry(vec![index::feature(), Arc::new(Blocks::default()), toy]);
    let doc = pipeline::doc(&registry, &["\\emph{a}", "plain", "\\emph{b} \\emph{c}"]);
    let plan = pipeline::plan(&registry, &["t.walked", "flm.blocks.structured"]);
    let _run = pipeline::run(&doc, &plan, &RunOptions::sequential());
    assert_eq!(recorded(&seen), ["tree#0 m", "tree#2 m", "tree#2 m"]);
}

#[test]
fn the_index_projects_its_node_references_per_tree() {
    let run = indexed(&registry(vec![emph("m")]), &["x"]);
    let projected = projection(&run, ELEMENT_INDEX);
    let SerialValue::Map(entries) = projected else { panic!("a map, not {projected:?}") };
    assert_eq!(entries[0].0, "flm.element-index");
    let SerialValue::Map(trees) = &entries[0].1 else { panic!("a map per tree") };
    assert_eq!(trees[0].0, "tree#0");
    let SerialValue::List(nodes) = &trees[0].1 else { panic!("a list of node references") };
    assert_eq!(nodes[0], SerialValue::Str("flm.container@1:1".into()));
}
