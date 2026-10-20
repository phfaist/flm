//! The HTML backend's artifact table (`flm_core::stdcore::backends::html::ARTIFACTS`):
//! a stage of the plan writes files the page links to into
//! `flm.output.html.artifacts`, and the final stage adds each of them to the
//! page's set after the page; with no writer the set holds the page alone.

mod common;

use std::sync::Arc;

use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{CapabilityDef, FlmFeature};
use flm_core::core::run::RunOptions;
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::stdcore::backends::html::{self, ARTIFACTS, DOCUMENT};
use flm_core::stdcore::features::graphics;
use flm_core::stdcore::{StdcoreConfig, package};

use common::pipeline::{doc, feature, registry, run, stage};

/// The capability the test stage provides, which the request demands so that
/// the stage is scheduled: an optional read schedules nothing by itself.
const FILES: &str = "t.files";

/// A feature whose one stage writes two artifacts into the artifact table:
/// what a stage copying the page's images beside it does.
fn files_feature() -> Arc<dyn FlmFeature> {
    feature("t", |b| {
        b.capability(CapabilityDef::new(FILES))?;
        b.stage(stage(
            StageDecl::new("t.write-files", Effect::Compute, Scope::Document)
                .provides(FILES)
                .writes(ARTIFACTS),
            Needs::new(),
            |_, out| {
                let set = out.table(ARTIFACTS).expect("declared");
                for name in ["figures/plot.png", "figures/photo.jpg"] {
                    set.insert(Artifact::bytes(ArtifactId::new("t", name), "image/png", vec![1, 2]))
                        .expect("distinct identities");
                }
            },
        ))
    })
}

/// The standard package's features but `graphics`, whose publish stage is the
/// table's writer in the standard package, then the test feature standing in
/// for it.
fn features() -> Vec<Arc<dyn FlmFeature>> {
    let mut features: Vec<Arc<dyn FlmFeature>> = package(StdcoreConfig::new())
        .features
        .into_iter()
        .filter(|feature| feature.info().name.as_str() != graphics::FEATURE)
        .collect();
    features.push(flm_core::math_passthrough::feature());
    features.push(files_feature());
    features
}

/// The page's set when the request demands `capabilities`.
fn output_set(capabilities: &[&str]) -> ArtifactSet {
    let registry = registry(features());
    let request = capabilities
        .iter()
        .fold(PlanRequest::new(), |request, capability| request.demand(*capability));
    let plan = Arc::new(Plan::build(&registry, &request).expect("the plan builds"));
    let document = doc(&registry, &["An image: see the figure."]);
    let run = run(&document, &plan, &RunOptions::sequential());
    run.table(DOCUMENT).expect("declared").expect("written").clone()
}

#[test]
fn the_final_stage_adds_every_artifact_of_the_table_after_the_page() {
    let set = output_set(&[DOCUMENT.name(), FILES]);
    let ids: Vec<String> = set.iter().map(|artifact| artifact.id.to_string()).collect();
    assert_eq!(set.len(), 3, "{ids:?}");
    let entry = set.entry().expect("the page is the entry");
    assert_eq!(entry.id, ArtifactId::new(html::ARTIFACT_OWNER, html::ARTIFACT_NAME));
    for name in ["figures/plot.png", "figures/photo.jpg"] {
        let artifact = set.get(&ArtifactId::new("t", name)).expect("the artifact is in the set");
        assert!(!artifact.entry);
        assert_eq!(artifact.media_type, "image/png");
    }
    assert!(set.violations().is_empty(), "{:?}", set.violations());
}

#[test]
fn with_no_writer_of_the_table_the_set_holds_the_page_alone() {
    let set = output_set(&[DOCUMENT.name()]);
    assert_eq!(set.len(), 1);
    assert!(set.entry().is_some());
}
