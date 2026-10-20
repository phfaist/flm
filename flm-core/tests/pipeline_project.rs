//! Tables through the projection (`flm_core::dump::project` over a run of
//! `flm_core::core::run`): a rewrite's trees table as tree references, an
//! artifact set as its artifacts and violations, a node-keyed table's
//! references resolved through the run's frozen trees, and an address that no
//! tree resolves.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::serialize::SerialValue;

use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};
use flm_core::core::run::RunOptions;
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl, TreesTable};
use flm_core::core::data::table::{Table, TableName};
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::lang::Flm;

use common::pipeline::{appending, doc, feature, plan, registry, run, stage, tree, Marks, MARKS};

const ONE: TableName<TreesTable> = TableName::new("t.one");
const ART: TableName<ArtifactSet> = TableName::new("t.art");

fn strings(values: &[&str]) -> SerialValue {
    SerialValue::List(values.iter().map(|v| SerialValue::Str(v.to_string())).collect())
}

fn entries(value: SerialValue) -> Vec<(String, SerialValue)> {
    match value {
        SerialValue::Map(entries) => entries,
        other => panic!("not a map: {other:?}"),
    }
}

fn list(value: &SerialValue) -> &[SerialValue] {
    match value {
        SerialValue::List(items) => items,
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn a_trees_table_projects_as_tree_references() {
    let registry =
        registry(vec![feature("t", |b| b.rewrite_stage(appending("t.rw", "t.one", "+")))]);
    let run = run(&doc(&registry, &["a", "b"]), &plan(&registry, &["t.one"]), &RunOptions::new());
    let trees: &TreesTable = run.table(ONE).unwrap().expect("published");
    let projector = Projector::new(ProjectOptions::new());
    assert_eq!(trees.project(&projector.table_projection(&run)), strings(&["tree#0", "tree#1"]));
    // Under its name, through the projector's convenience.
    assert_eq!(
        projector.table("t.one", trees, &run),
        SerialValue::Map(vec![("t.one".to_string(), strings(&["tree#0", "tree#1"]))])
    );
}

#[test]
fn an_artifact_set_projects_its_artifacts_and_violations() {
    let registry = registry(vec![feature("t", |b| {
        b.table(ART, "the artifacts")?;
        b.stage(stage(
            StageDecl::new("t.final", Effect::Compute, Scope::PerTree).writes(ART),
            Needs::new(),
            |cx, out| {
                let index = cx.tree_id().unwrap().index();
                let id = ArtifactId::new("t", format!("doc{index}"));
                let bytes = format!("tree {index}").into_bytes();
                let artifact = Artifact::bytes(id, "text/plain", bytes).as_entry();
                out.table(ART).unwrap().insert(artifact).expect("one entry per piece");
            },
        ))
    })]);
    // Two per-tree pieces each claim the entry: the fold keeps the first and
    // records the second as a violation.
    let run = run(&doc(&registry, &["a", "b"]), &plan(&registry, &["t.art"]), &RunOptions::new());
    let set: &ArtifactSet = run.table(ART).unwrap().expect("written");
    let projector = Projector::new(ProjectOptions::new());
    let projected = entries(set.project(&projector.table_projection(&run)));
    assert_eq!(projected[0].0, "artifacts");
    let artifacts = list(&projected[0].1);
    assert_eq!(artifacts.len(), 2);
    assert_eq!(
        artifacts[0],
        SerialValue::Map(vec![
            ("id".to_string(), SerialValue::Str("t:doc0".to_string())),
            ("entry".to_string(), SerialValue::Bool(true)),
            ("media_type".to_string(), SerialValue::Str("text/plain".to_string())),
            ("data".to_string(), SerialValue::Bytes(b"tree 0".to_vec())),
        ])
    );
    assert_eq!(projected[1].0, "violations");
    assert_eq!(list(&projected[1].1).len(), 1);
    assert_eq!(set.violations().len(), 1);
}

#[test]
fn a_node_keyed_table_resolves_its_references_through_the_frozen_trees() {
    let registry = registry(vec![feature("t", |b| {
        b.table(MARKS, "node address → text")?;
        b.stage(stage(
            StageDecl::new("t.mark", Effect::Compute, Scope::PerTree).writes(MARKS),
            Needs::new(),
            |cx, out| {
                let tree = cx.my_tree().expect("present");
                for node in tree.root().descendants() {
                    if let Some(chars) = node.chars() {
                        let addr = cx.addr(&node).expect("a node of this tree");
                        out.table(MARKS).unwrap().0.insert(addr, chars.to_string());
                    }
                }
            },
        ))
    })]);
    let doc = doc(&registry, &["ab", "c"]);
    let run = run(&doc, &plan(&registry, &["t.marks"]), &RunOptions::new());
    let marks: &Marks = run.table(MARKS).unwrap().expect("written");
    let projector = Projector::new(ProjectOptions::new());
    assert_eq!(
        marks.project(&projector.table_projection(&run)),
        SerialValue::List(vec![strings(&["flm.text@1:1", "ab"]), strings(&["flm.text@1:1", "c"])])
    );
}

#[test]
fn an_address_no_tree_resolves_renders_as_unresolvable() {
    let registry = registry(Vec::new());
    let value = tree(&registry, "v");
    let mut marks = Marks::default();
    marks.0.insert(NodeAddr::new(TreeId::value(0), value.root().id()), "v".to_string());
    let projector = Projector::new(ProjectOptions::new());
    // No source holds a value's tree: the reference says so instead of guessing.
    let none: BTreeMap<TreeId, Arc<NodeTree<Flm>>> = BTreeMap::new();
    let projected = marks.project(&projector.table_projection(&none));
    let reference = match &list(&projected)[0] {
        SerialValue::List(entry) => entry[0].clone(),
        other => panic!("not an entry: {other:?}"),
    };
    assert_eq!(reference, SerialValue::Str(format!("?@value#0/{}", value.root().id().index())));
    // A consumer holding the value projects it inline, and may resolve through it.
    let held: BTreeMap<TreeId, Arc<NodeTree<Flm>>> =
        [(TreeId::value(0), Arc::clone(&value))].into();
    let projected = marks.project(&projector.table_projection(&held));
    let reference = match &list(&projected)[0] {
        SerialValue::List(entry) => entry[0].clone(),
        other => panic!("not an entry: {other:?}"),
    };
    assert_eq!(reference, SerialValue::Str("flm.container@1:1".to_string()));
    assert_eq!(list(&projector.tree(&value)).len(), 1, "the value projects inline");
}
