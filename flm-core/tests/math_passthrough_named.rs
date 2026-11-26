//! The four formula rules of `math-passthrough` that write a formula as LaTeX
//! (`flm_core::math_passthrough::{math_rule_named, environment_rule_named}`,
//! `flm_core::math_passthrough::latex::{math_rule_named,
//! environment_rule_named}`), registered under rule names of its own by a
//! feature other than `math-passthrough`, in a registry without
//! `math-passthrough`: a stand-in processor that declares its commands of
//! math mode by name and a check stage providing `flm.math.content`. Its
//! registry entries are the library's rules but for their names. The
//! `latex-rendered` and `latex-recomposed` outputs of a document with a math
//! group, an environment with a label and a tag, and a text command inside a
//! formula are those of the same document under `math-passthrough`, and the
//! plan's warning on each of these outputs names the stand-in processor,
//! with the texts `math-passthrough` declares.

mod common;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::store::StageOut;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::{Overrides, PlanCx};
use flm_core::core::registry::{FlmFeature, FlmRegistry, PackageEntry};
use flm_core::core::run::{Run, StageCx};
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ElementDef, SyntaxDef};
use flm_core::math_passthrough::{self, latex};
use flm_core::stdlib::backends::latex::construct::{DEFINITION, MACRO, RAW};
use flm_core::stdlib::backends::latex::{LatexDefinitionPayload, LatexMacroPayload, LatexRawPayload};
use flm_core::stdlib::backends::{latex_recomposed, latex_rendered};
use flm_core::stdlib::features::math::CONTENT;
use flm_core::stdlib::features::numbering::{COUNTERS, Counters};
use flm_core::stdlib::features::refs::{ANCHORS, Anchors, SAFE_LABELS, SafeLabels};
use flm_core::stdlib::{StdlibConfig, VALIDATED, add_features};
use flm_core::techy::core::node::NodeRef;
use flm_core::techy::latexlike::Mode;

use common::pipeline;
use common::stdlib::{assert_only_unchecked_output, registry, registry_over, run_workflow, unchecked_outputs};

/// The stand-in processor's name.
const PROCESSOR: &str = "standin";

/// A math group, an environment with a label and a tag, a text command
/// inside a formula, and a reference to the labeled line, over the
/// commands the stand-in processor declares.
const DOCUMENT: &str = "Let \\(\\alpha + \\text{if } x\\) and \\[\\beta\\] hold.\n\n\
                        \\begin{align}\n  a \\alpha \\label{eq:a} \\\\\n  b \\text{ for all } \\beta \\tag{T}\n\
                        \\end{align}\n\nSee \\eqref{eq:a}.";

/// The stand-in processor's check: it provides `flm.math.content` and
/// accepts every formula.
struct StandInCheck;

impl Stage for StandInCheck {
    fn decl(&self) -> StageDecl {
        StageDecl::new("standin.check", Effect::Check, Scope::PerTree)
            .provides(CONTENT)
            .docs("the stand-in processor's check of the mathematics")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
}

/// The element of the stand-in processor's commands.
const COMMAND: &str = "standin.command";

/// The rule of the stand-in processor's commands: the node's source. The
/// rules of the formulas write a formula whole, so a plan selects it without
/// calling it.
struct Source;

impl Lower for Source {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(vec![lw.text(cx, node.span_content())?]))
    }
}

/// The names the stand-in processor registers the four rules under, in the
/// order of the library's [`LIBRARY_RULES`].
const RULES: [&str; 4] = [
    "flm.math.lower.standin",
    "flm.math.environment.lower.standin",
    "flm.math.lower.standin-latex-recomposed",
    "flm.math.environment.lower.standin-latex-recomposed",
];

/// The names of the four rules under `math-passthrough`.
const LIBRARY_RULES: [&str; 4] = [
    math_passthrough::MATH_RULE,
    math_passthrough::ENVIRONMENT_RULE,
    latex::MATH_LATEX_RULE,
    latex::ENVIRONMENT_LATEX_RULE,
];

/// The stand-in processor: `\alpha` and `\beta` declared in math mode only,
/// with the rule of their element, its check, and the four rules of `math-passthrough` under names of its
/// own, with the peer declarations their documentation lists.
fn processor() -> Arc<dyn FlmFeature> {
    pipeline::feature(PROCESSOR, |b| {
        let syntax = SyntaxDef::Macro { args: Vec::new() };
        b.element(ElementDef::new(PROCESSOR, ["alpha", "beta"], syntax, common::element(COMMAND)).modes([Mode::Math]))?;
        b.lowering_rule(LoweringRule::new(
            "standin.command.lower.source",
            COMMAND,
            LoweringDecl::new(),
            Arc::new(Source),
            DefaultMark::Everywhere,
        ))?;
        b.stage(Arc::new(StandInCheck))?;
        b.collection_member(VALIDATED, CONTENT)?;
        b.peer_table::<Anchors>(ANCHORS.name(), "the anchors table")?;
        b.peer_table::<Counters>(COUNTERS.name(), "the counter table")?;
        b.peer_backend(latex_recomposed::BACKEND, "the recomposed LaTeX")?;
        b.peer_element::<LatexMacroPayload>(MACRO, "the macro construct")?;
        b.peer_element::<LatexRawPayload>(RAW, "the raw construct")?;
        b.peer_element::<LatexDefinitionPayload>(DEFINITION, "the definition construct")?;
        b.peer_table::<SafeLabels>(SAFE_LABELS.name(), "the safe-labels table")?;
        b.lowering_rule(math_passthrough::math_rule_named(RULES[0], DefaultMark::Everywhere))?;
        b.lowering_rule(math_passthrough::environment_rule_named(RULES[1], DefaultMark::Everywhere))?;
        b.lowering_rule(latex::math_rule_named(RULES[2]))?;
        b.lowering_rule(latex::environment_rule_named(RULES[3]))
    })
}

/// The standard package and the stand-in processor, without
/// `math-passthrough`.
fn standin() -> Arc<FlmRegistry> {
    let mut features: Vec<Arc<dyn FlmFeature>> = add_features(PackageEntry::new("std"), StdlibConfig::new()).features;
    features.push(processor());
    registry_over(features)
}

/// The run of `workflow` over [`DOCUMENT`], asserting that it reported
/// nothing but the Warnings of its plan's unchecked output.
fn render(registry: &Arc<FlmRegistry>, workflow: &str) -> Run {
    let run = run_workflow(registry, workflow, &[DOCUMENT], &Overrides::none());
    assert_only_unchecked_output(&run);
    run
}

/// Every artifact of `set`: its identity and its bytes as text.
fn artifacts(set: &ArtifactSet) -> Vec<(String, String)> {
    set.iter()
        .map(|artifact| {
            let bytes = artifact.as_bytes().expect("bytes");
            (artifact.id.to_string(), String::from_utf8(bytes.to_vec()).expect("UTF-8"))
        })
        .collect()
}

#[test]
fn the_registry_holds_no_math_passthrough_and_declares_the_commands_by_name() {
    let registry = standin();
    assert!(registry.feature(math_passthrough::FEATURE).is_none());
    assert!(registry.feature(PROCESSOR).is_some());
    let fragment = common::parse(registry.lang(), DOCUMENT);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
}

#[test]
fn the_registered_rules_are_the_library_s_under_the_processor_s_names() {
    let (standin, library) = (standin(), registry(StdlibConfig::new()));
    for (ours, theirs) in RULES.iter().zip(LIBRARY_RULES) {
        let ours = standin.rules().iter().find(|entry| entry.rule.name.as_str() == *ours).expect("registered");
        let theirs = library.rules().iter().find(|entry| entry.rule.name.as_str() == theirs).expect("registered");
        assert_eq!(ours.owner.as_str(), PROCESSOR);
        let (ours, theirs) = (&ours.rule, &theirs.rule);
        assert_eq!((&ours.element, &ours.docs, &ours.default), (&theirs.element, &theirs.docs, &theirs.default));
        assert_eq!(format!("{:?}", ours.decl), format!("{:?}", theirs.decl), "{}", ours.name);
    }
}

#[test]
fn the_latex_outputs_are_those_of_math_passthrough() {
    let (standin, library) = (standin(), registry(StdlibConfig::new()));
    // The recomposed LaTeX writes the line's label in `\flmLabels`; the
    // rendered LaTeX writes a tag for every numbered line.
    for (workflow, table, label) in [
        (latex_rendered::WORKFLOW, latex_rendered::DOCUMENT, "\\tag*{"),
        (latex_recomposed::WORKFLOW, latex_recomposed::DOCUMENT, "\\flmLabels{"),
    ] {
        let (ours, theirs) = (render(&standin, workflow), render(&library, workflow));
        let (ours, theirs) = (ours.table(table).expect("declared"), theirs.table(table).expect("declared"));
        let (ours, theirs) = (ours.expect("written"), theirs.expect("written"));
        assert_eq!(artifacts(ours), artifacts(theirs), "{workflow}");
        let entry = String::from_utf8_lossy(ours.entry().and_then(|entry| entry.as_bytes()).expect("the entry"));
        for written in ["\\alpha", "\\beta", "for all", "(T)", label] {
            assert!(entry.contains(written), "{workflow}: {written} in {entry}");
        }
    }
}

#[test]
fn the_plan_s_warning_names_the_registering_feature_with_the_library_s_texts() {
    let (standin, library) = (standin(), registry(StdlibConfig::new()));
    for workflow in [latex_rendered::WORKFLOW, latex_recomposed::WORKFLOW] {
        let ours = unchecked_outputs(render(&standin, workflow).plan());
        let theirs = unchecked_outputs(render(&library, workflow).plan());
        assert_eq!(ours.len(), 1, "{workflow}: {ours:?}");
        assert_eq!(theirs.len(), 1, "{workflow}: {theirs:?}");
        let (backend, feature, written, reader) = &ours[0];
        assert_eq!(feature, PROCESSOR, "{workflow}");
        assert_eq!(theirs[0].1, math_passthrough::FEATURE, "{workflow}");
        assert_eq!((backend, written, reader), (&theirs[0].0, &theirs[0].2, &theirs[0].3), "{workflow}");
    }
}
