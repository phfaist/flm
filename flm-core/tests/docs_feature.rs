//! The cargo feature `docs`: with it, the registry keeps the documentation
//! texts that features give their declarations, and the feature
//! configuration values and the declarations read from settings data keep
//! theirs; without it, every such text is empty, and a feature's title is
//! kept. Either way, the registry digest does not depend on the texts.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, StyleDef,
};
use flm_core::core::settings::cascade::workflow_def;
use flm_core::core::settings::named::NamedEntry;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ScopedPackageDef, TackOnField};
use flm_core::model::names::Name;
use flm_core::stdcore::features::cells::CellStyle;
use flm_core::stdcore::features::headings::{HeadingDecl, HeadingsConfig};
use flm_core::stdcore::features::numbering::CounterDecl;
use flm_core::stdcore::features::userdefs::{DefinitionData, DefinitionKind};

use common::{macro_def, macro_with};

/// A feature that gives the same documentation text to each kind of
/// declaration it makes.
struct Documented {
    text: &'static str,
}

impl FlmFeature for Documented {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.doc", "Documented").docs(self.text)
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.argument_role("t.doc.role", self.text)?;
        let field = TackOnField::new("tag", vec![ArgDef::named("tag", ArgStdKind::CharsGroup)]).docs(self.text);
        b.element(
            macro_with(
                "t.doc",
                "thing",
                vec![
                    ArgDef::named("x", ArgStdKind::Mandatory).docs(self.text).role("t.doc.role"),
                    ArgDef::named("fields", ArgStdKind::TackOn { fields: vec![field] }),
                ],
            )
            .docs(self.text),
        )?;
        b.scoped_package(ScopedPackageDef {
            owner: "t.doc".into(),
            name: Name::from("t.doc.package"),
            elements: vec![macro_def("t.doc", "inner", 0).docs(self.text)],
            docs: String::from(self.text),
        })?;
        b.style_def(StyleDef {
            id: Name::from("t.doc.style"),
            content: vec![DefContent::Inline(String::from("p {}"))],
            docs: String::from(self.text),
        })?;
        Ok(())
    }
}

fn registry(text: &'static str) -> Arc<FlmRegistry> {
    common::pipeline::registry(vec![Arc::new(Documented { text })])
}

/// Every documentation text the feature gave, as the registry keeps it.
fn texts(registry: &FlmRegistry) -> Vec<String> {
    let feature = registry.features().iter().find(|info| info.name == "t.doc").unwrap();
    let thing = registry.declarations().elements_of("t.doc").next().unwrap();
    let package = registry.declarations().scoped_package("t.doc.package").unwrap();
    let field = match &thing.syntax {
        flm_core::lang::mint::SyntaxDef::Macro { args } => args[1].kind.tack_on_fields().unwrap()[0].docs.clone(),
        other => panic!("a macro, not {other:?}"),
    };
    let argument = match &thing.syntax {
        flm_core::lang::mint::SyntaxDef::Macro { args } => args[0].docs.clone(),
        other => panic!("a macro, not {other:?}"),
    };
    let style = registry.styles().iter().find(|(_, style)| style.id == "t.doc.style").unwrap();
    vec![
        feature.docs.clone(),
        registry.argument_role_named("t.doc.role").unwrap().docs.clone(),
        thing.docs.clone(),
        argument,
        field,
        package.docs.clone(),
        package.elements[0].docs.clone(),
        style.1.docs.clone(),
    ]
}

#[cfg(feature = "docs")]
#[test]
fn the_registry_keeps_every_documentation_text() {
    let registry = registry("The text.");
    assert_eq!(texts(&registry), vec![String::from("The text."); 8]);
}

#[cfg(not(feature = "docs"))]
#[test]
fn the_registry_keeps_no_documentation_text_and_keeps_the_title() {
    let registry = registry("The text.");
    assert_eq!(texts(&registry), vec![String::new(); 8]);
    let feature = registry.features().iter().find(|info| info.name == "t.doc").unwrap();
    assert_eq!(feature.title, "Documented");
}

#[test]
fn the_digest_does_not_depend_on_the_documentation_texts() {
    assert_eq!(registry("One text.").digest(), registry("Another text.").digest());
}

/// The documentation texts of feature configuration values and of
/// declarations read from settings data: a heading kind built in Rust and one
/// read from data, a counter, a cell style, and a user definition and a
/// workflow read from data.
fn configuration_texts(text: &str) -> Vec<String> {
    let read_kind = HeadingDecl::read_entry("aside", serde_json::json!({ "level": 1, "docs": text })).unwrap();
    let definition: DefinitionData = serde_json::from_value(serde_json::json!({ "docs": text })).unwrap();
    let workflow_docs = serde_json::Value::from(text);
    vec![
        HeadingDecl::new("aside", 1).docs(text).docs,
        read_kind.docs,
        CounterDecl::new("t.doc.counter", "t.doc.style").docs(text).docs,
        CellStyle::custom("orange", text).docs,
        definition.definitions(DefinitionKind::Macro, "m").remove(0).docs,
        workflow_def("t.doc.flow", [("docs", &workflow_docs)]).docs,
    ]
}

#[cfg(feature = "docs")]
#[test]
fn the_configuration_values_and_the_settings_data_keep_their_texts() {
    assert_eq!(configuration_texts("The text."), vec![String::from("The text."); 6]);
}

#[cfg(not(feature = "docs"))]
#[test]
fn the_configuration_values_and_the_settings_data_keep_no_text() {
    assert_eq!(configuration_texts("The text."), vec![String::new(); 6]);
}

#[test]
fn the_digest_input_of_the_headings_configuration_does_not_depend_on_the_texts() {
    // The field is set directly, so that the texts differ in both builds.
    let described = |text: &str| {
        let mut config = HeadingsConfig::new();
        config.headings[0].docs = String::from(text);
        config.digest_bytes()
    };
    assert_eq!(described("One text."), described("Another text."));
}
