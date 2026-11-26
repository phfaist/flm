//! The formulas on the backends `latex-rendered` and `latex-recomposed`: the
//! library's own formula rules, registered by the feature under names of its
//! own. The outputs of the documents the tests of every output share
//! (`common::documents`) are those of the same documents under
//! `math-passthrough`, in a registry that differs in the processor alone; and
//! the plans keep the warning of the library's declarations on the LaTeX
//! outputs only, naming the feature `math-techxt`.

mod common;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::TableName;
use flm_core::core::plan::PlanWarning;
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::stdlib::backends::{html, latex_recomposed, latex_rendered, text};
use flm_core::math_passthrough;
use flm_core::stdlib::{self, StdlibConfig};

use common::{FEATURE, diagnostics, documents, registry, run, selected_rule, workflow_plan};

/// The two LaTeX outputs: the workflow, and the table of its document.
const LATEX: [(&str, TableName<ArtifactSet>); 2] =
    [(latex_rendered::WORKFLOW, latex_rendered::DOCUMENT), (latex_recomposed::WORKFLOW, latex_recomposed::DOCUMENT)];

/// The registry holding the standard package and `math-passthrough`, the
/// library's processor, in place of the extension.
fn passthrough() -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("the registry builds: {}", error.render()))
}

/// Every artifact of the document table `table` of `run`: its identity and its
/// bytes as text.
fn artifacts(run: &Run, table: TableName<ArtifactSet>) -> Vec<(String, String)> {
    let set = run.table(table).expect("declared").unwrap_or_else(|| panic!("written: {:?}", diagnostics(run)));
    set.iter()
        .map(|artifact| {
            let bytes = artifact.as_bytes().expect("bytes");
            (artifact.id.to_string(), String::from_utf8(bytes.to_vec()).expect("UTF-8"))
        })
        .collect()
}

/// The lines of `ours` and `theirs` that differ, numbered from 1, for a
/// failure's message.
fn differing_lines(ours: &str, theirs: &str) -> String {
    let (ours, theirs): (Vec<_>, Vec<_>) = (ours.lines().collect(), theirs.lines().collect());
    (0..ours.len().max(theirs.len()))
        .filter(|&i| ours.get(i) != theirs.get(i))
        .map(|i| format!("line {}:\n  math-techxt:      {:?}\n  math-passthrough: {:?}", i + 1, ours.get(i), theirs.get(i)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A plan's warning of an output that nothing checks: its backend, its
/// feature, the text written and the reader.
type Unchecked = (String, String, String, String);

/// The warnings of the plan of `workflow` under `registry` of an output that
/// nothing checks, and the plan's other warnings, as their debug form.
fn plan_warnings(registry: &Arc<FlmRegistry>, workflow: &str) -> (Vec<Unchecked>, Vec<String>) {
    let plan = workflow_plan(registry, workflow);
    let (mut unchecked, mut others) = (Vec::new(), Vec::new());
    for warning in plan.warnings() {
        match warning {
            PlanWarning::UncheckedOutput { backend, feature, written, reader, .. } => {
                unchecked.push((backend.to_string(), feature.to_string(), written.clone(), reader.clone()));
            }
            other => others.push(format!("{other:?}")),
        }
    }
    (unchecked, others)
}

#[test]
fn the_plans_select_the_library_s_rules_under_the_feature_s_names() {
    let registry = registry();
    for (workflow, backend) in [
        (latex_rendered::WORKFLOW, "latex-rendered"),
        (latex_recomposed::WORKFLOW, "latex-recomposed"),
        (text::WORKFLOW, "text"),
        (html::WORKFLOW, "html"),
    ] {
        let rule = |element| selected_rule(&registry, workflow, backend, element);
        assert_eq!(rule("flm.math"), Some(format!("math-techxt.math.lower.{backend}")), "{workflow}");
        assert_eq!(rule("flm.math.environment"), Some(format!("math-techxt.math-environment.lower.{backend}")), "{workflow}");
    }
}

#[test]
fn the_latex_outputs_are_those_of_math_passthrough() {
    let (ours, theirs) = (registry(), passthrough());
    for document in documents::all() {
        for (workflow, table) in LATEX {
            let (ours, theirs) = (run(&ours, workflow, &document), run(&theirs, workflow, &document));
            let (ours, theirs) = (artifacts(&ours, table), artifacts(&theirs, table));
            let ids = |set: &[(String, String)]| set.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
            assert_eq!(ids(&ours), ids(&theirs), "{workflow}: {document:?}");
            for ((id, ours), (_, theirs)) in ours.iter().zip(&theirs) {
                assert!(ours == theirs, "{workflow}, {id}, {document:?}:\n{}", differing_lines(ours, theirs));
            }
        }
    }
    // The formulas are written from their nodes: the environment's matrix,
    // its author's tag and its text command reach both LaTeX outputs.
    for (workflow, table) in LATEX {
        let run = run(&ours, workflow, documents::ENVIRONMENT);
        let set = run.table(table).expect("declared").expect("written");
        let entry = String::from_utf8_lossy(set.entry().and_then(|entry| entry.as_bytes()).expect("the entry"));
        for expected in ["\\begin{pmatrix} 1 & 2 \\\\ 30 & 4 \\end{pmatrix}", "\\text{c}", "(T)"] {
            assert!(entry.contains(expected), "{workflow}: {expected} in {entry}");
        }
    }
}

#[test]
fn the_plans_warn_of_the_latex_outputs_alone_naming_the_feature() {
    let (ours, theirs) = (registry(), passthrough());
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        assert_eq!(plan_warnings(&ours, workflow), (Vec::new(), Vec::new()), "{workflow}");
    }
    for (workflow, _) in LATEX {
        let (unchecked, others) = plan_warnings(&ours, workflow);
        assert_eq!(others, Vec::<String>::new(), "{workflow}");
        assert_eq!(unchecked.len(), 1, "{workflow}: {unchecked:?}");
        let (backend, feature, written, reader) = &unchecked[0];
        assert_eq!((backend.as_str(), feature.as_str()), (workflow, FEATURE));
        // The texts are the library's declaration's, as `math-passthrough`
        // carries them.
        let (library, _) = plan_warnings(&theirs, workflow);
        assert_eq!(library.len(), 1, "{workflow}: {library:?}");
        assert_eq!((written, reader), (&library[0].2, &library[0].3), "{workflow}");
    }
}
