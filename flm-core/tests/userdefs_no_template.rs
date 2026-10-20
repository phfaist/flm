//! A user definition that carries no template at all: it is refused when the
//! registry is built, and a use of it recovers with a diagnostic of its own
//! rather than silently expanding to nothing.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{BadDefinition, Definition, DefinitionProblem};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};

use common::lowering::text as text_of;
use common::stdcore::{identifiers, run_workflow};

/// A registry over the `userdefs` feature carrying `definitions`, the lowering
/// machinery, the text backend and the `blocks` feature, together with the
/// diagnostics its build produced.
fn registry_with(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.feature(lowering::feature());
    builder.feature(flm_core::stdcore::backends::pieces::feature());
    builder.feature(text::feature());
    builder.feature(check_feature());
    builder.feature(Arc::new(Blocks::default()));
    builder.feature(Arc::new(Userdefs::new(UserdefsConfig::new().definitions(definitions))));
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_empty_content.
#[test]
fn a_definition_with_no_template_is_refused_at_build_and_its_use_recovers() {
    let registry = registry_with(vec![Definition::macro_("mymacro", Vec::<ArgDef>::new())]);

    let refused: Vec<&BadDefinition> = registry
        .build_diagnostics()
        .iter()
        .filter_map(|diagnostic| diagnostic.data().downcast_ref::<BadDefinition>())
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].name, "\\mymacro");
    assert_eq!(refused[0].problem, DefinitionProblem::NoTemplate);

    let document = "Test \\mymacro here\n";
    let run = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    // The definition stays registered and uncompiled, so its use site is met by
    // the expansion and recovered there, naming the refused definition.
    assert_eq!(identifiers(&run), ["flm.userdefs.bad-definition"]);
    // The recovery prints the use site as it was written, so nothing is lost.
    assert_eq!(text_of(&run), "Test \\mymacro here\n");
}
