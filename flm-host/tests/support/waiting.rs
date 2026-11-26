//! A compiled package whose stage waits for its test, so that a test acts while a build
//! runs without waiting on time. Shared by the tests of the live preview, each including
//! this file as a module.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use flm_core::core::registry::PackageFactory;

/// The channels between a test and the stage of [`waiting_package`].
pub struct Waiting {
    /// The stage tells here that it runs.
    pub running: Sender<()>,
    /// The stage returns once a message arrives here, or once the test's sender is gone.
    pub release: Mutex<Receiver<()>>,
    /// The stage tells here, as it returns, whether its build was cancelled.
    pub cancelled: Sender<bool>,
}

/// A compiled package `waiting`, whose workflow `waiting` extends `html` and demands what
/// its stage `waiting.wait` provides; the stage returns only once `waiting` says so.
pub fn waiting_package(waiting: Arc<Waiting>) -> PackageFactory {
    use flm_core::core::data::store::StageOut;
    use flm_core::core::plan::PlanCx;
    use flm_core::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, WorkflowDef};
    use flm_core::core::run::StageCx;
    use flm_core::core::stage::{Effect, Needs, Scope as StageScope, Stage, StageDecl};

    struct Wait(Arc<Waiting>);
    impl Stage for Wait {
        fn decl(&self) -> StageDecl {
            StageDecl::new("waiting.wait", Effect::Compute, StageScope::Document).provides("waiting.waited")
        }
        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            Needs::new()
        }
        fn run(&self, cx: &StageCx<'_>, _out: &mut StageOut<'_>) {
            let _ = self.0.running.send(());
            let _ = self.0.release.lock().unwrap().recv();
            let _ = self.0.cancelled.send(cx.progress().cancelled());
        }
    }
    struct Feature(Arc<Waiting>);
    impl FlmFeature for Feature {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("waiting", "A stage that waits for the test")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.capability(CapabilityDef::new("waiting.waited"))?;
            b.stage(Arc::new(Wait(Arc::clone(&self.0))))?;
            b.workflow(WorkflowDef::new("waiting").extends("html").demand("waiting.waited"))
        }
    }
    PackageFactory::new("waiting", move |_| Ok(PackageEntry::new("waiting").feature(Arc::new(Feature(Arc::clone(&waiting))))))
        .unwrap()
        .uses("std")
}
