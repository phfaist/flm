//! A feature registering definitions of its own, as elements of its own
//! expanded by a hook of its own, through what the library offers for it: the
//! element name under the feature's prefix (`element_name_under`), the element
//! declaration with another `Expand` hook (`UserDefinition::element_def_with`),
//! the use site's compiled templates or the condition it is reported with
//! (`UserDefinition::variant_for_use`), and the `expand-in-tree` rule for the
//! feature's element (`expand_in_tree_rule_for`) — written against the public
//! interface alone.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, LateInit, LateInitError,
};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{
    BadDefinition, Definition, DefinitionBindings, DefinitionProblem, Expand, ExpandCx,
    ExpandError, ExpandPolicy, Expander, Expansion, PlaceholderBindings, PlaceholderTarget,
    PlaceholderValue, TemplateLanguage, UserDefinition,
};
use flm_core::lang::{BuiltLang, Flm, FlmDiagnostics};
use flm_core::model::element::element_value;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::userdefs::{
    element_name_under, expand_in_tree_rule, expand_in_tree_rule_for,
};
use flm_core::stdcore::StdcoreConfig;
use techy::core::node::{NodeRef, NodeSlice};
use techy::error::Diagnostic;
use techy::latexlike::Mode;

use common::parse;
use common::stdcore::{identifiers, package_features_but, run_workflow};

/// The bindings of the feature's hook: `#{mark}` stands for nothing, and every
/// other reference means what it means for a user definition.
struct MarkBindings<'t> {
    base: DefinitionBindings<'t>,
}

impl PlaceholderBindings for MarkBindings<'_> {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        match target {
            PlaceholderTarget::Extra(name) if name == "mark" => PlaceholderValue::NoValue,
            _ => self.base.value(target, substitution),
        }
    }
}

/// The feature's own expansion hook.
struct Marked {
    definition: Arc<UserDefinition>,
}

impl Expand for Marked {
    fn policy(&self) -> ExpandPolicy {
        self.definition.definition().expand
    }

    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        let variant = match self.definition.variant_for_use(&node) {
            Ok(variant) => variant,
            Err(condition) => {
                let diagnostic = Diagnostic::error(condition, node.span().clone());
                return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
            }
        };
        let bindings = MarkBindings { base: DefinitionBindings::new(node, variant) };
        Ok(Expansion::Emit(xp.instantiate(cx, node, variant.template(), &bindings)?))
    }
}

/// Compiles the feature's definitions when the registry is built.
struct Compile {
    definitions: Vec<Arc<UserDefinition>>,
}

impl LateInit for Compile {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let Ok(language) = TemplateLanguage::new(lang) else { return Ok(()) };
        for definition in &self.definitions {
            let (Ok(reported) | Err(reported)) = definition.compile_with(lang, &language);
            for diagnostic in reported {
                diags.push(diagnostic);
            }
        }
        Ok(())
    }
}

/// A feature `ext` registering each of its definitions under the prefix `ext`,
/// expanded by its own hook in every workflow.
struct Ext {
    definitions: Vec<Definition>,
}

impl FlmFeature for Ext {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("ext", "A feature with definitions of its own")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        let mut definitions = Vec::new();
        for definition in &self.definitions {
            let value = UserDefinition::new(definition.clone().expand(ExpandPolicy::Always));
            let element = element_name_under("ext", value.definition());
            let hook: Arc<dyn Expand> = Arc::new(Marked { definition: Arc::clone(&value) });
            b.element(value.element_def_with(owner.clone(), element.clone(), hook))?;
            b.lowering_rule(expand_in_tree_rule_for(element, value.definition()))?;
            definitions.push(value);
        }
        b.late_init(Arc::new(Compile { definitions }))?;
        Ok(())
    }
}

/// `\tw{…}`, with the template `<#1#{mark}>`.
fn tw() -> Definition {
    Definition::macro_("tw", vec![ArgDef::mandatory("a")])
        .content("<#1#{mark}>")
        .extra_placeholder("mark", false)
}

/// The standard package's features and the feature of the package
/// `math-passthrough`, then `ext` over `definitions`.
fn features(definitions: Vec<Definition>) -> Vec<Arc<dyn FlmFeature>> {
    let mut features = flm_core::stdcore::package(StdcoreConfig::new()).features;
    features.push(flm_core::math_passthrough::feature());
    features.push(Arc::new(Ext { definitions }));
    features
}

#[test]
fn the_element_is_named_under_the_features_prefix_and_carries_the_definition() {
    let registry = common::pipeline::registry(features(vec![tw()]));
    assert!(registry.element_value("ext.macro.tw").unwrap().is_some());
    let fragment = parse(registry.lang(), r"\tw{A}");
    let value = fragment
        .tree()
        .iter_storage_order()
        .find_map(|node| element_value::<UserDefinition>(&node))
        .expect("the definition on the use site's element");
    assert_eq!(value.definition().name.as_str(), "tw");
}

#[test]
fn a_definition_of_formulas_alone_is_named_with_the_math_segment() {
    let definition = Definition::macro_("tw", Vec::new()).math("x");
    assert_eq!(element_name_under("ext", &definition), "ext.macro.math.tw");
}

#[test]
fn the_features_own_hook_expands_every_use_before_lowering() {
    let registry = common::pipeline::registry(features(vec![tw()]));
    let run = run_workflow(&registry, text::WORKFLOW, &[r"say \tw{A}"], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = common::lowering::text(&run);
    assert!(out.contains("say <A>"), "{out}");
}

#[test]
fn a_use_where_the_definition_is_not_in_force_is_reported_as_for_every_user_definition() {
    let formula = Definition::macro_("tw", vec![ArgDef::mandatory("a")])
        .content(r"\alpha#1#{mark}")
        .extra_placeholder("mark", false);
    let registry = common::pipeline::registry(features(vec![formula]));
    let run = run_workflow(&registry, text::WORKFLOW, &[r"say \tw{A}"], &Overrides::none());
    let reported: Vec<BadDefinition> = run
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| diagnostic.data().downcast_ref::<BadDefinition>().cloned())
        .collect();
    assert_eq!(reported.len(), 1, "{:?}", identifiers(&run));
    assert_eq!(reported[0].problem, DefinitionProblem::NotInForceInMode { mode: Mode::Text });
}

#[test]
fn a_registry_without_the_userdefs_feature_does_not_build() {
    // The rule requires the table of the `userdefs` feature's stage that
    // expands the `always` definitions, which no other feature declares.
    let mut features = package_features_but(StdcoreConfig::new(), "userdefs");
    features.push(Arc::new(Ext { definitions: vec![tw()] }));
    let _ = common::pipeline::registry_error(features);
}

#[test]
fn the_rule_for_an_element_is_the_rule_of_the_definitions_own_element_renamed() {
    for policy in [ExpandPolicy::Always, ExpandPolicy::PerPlan] {
        let definition = tw().expand(policy);
        let own = expand_in_tree_rule(&definition);
        let other = expand_in_tree_rule_for("ext.macro.tw", &definition);
        assert_eq!(other.name, "ext.macro.tw.lower.expand-in-tree");
        assert_eq!(other.element, "ext.macro.tw");
        assert_eq!(other.decl.requires, own.decl.requires, "{policy:?}");
    }
}
