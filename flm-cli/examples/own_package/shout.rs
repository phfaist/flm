//! The package `shout` of the example `own_package`: one feature, also named `shout`, which
//! declares the macro `\shout{text}`. The macro is a user definition, the kind of macro that
//! the `userdefs` feature of the standard package declares for an author: its template
//! `\textbf{#{text}}` writes the argument `text` in bold, so that `\shout{listen}` reads as
//! `\textbf{listen}`. A document gets the macro by listing the package in its front matter
//! (`use: [shout]`).
//!
//! The feature registers three things for the definition, with the functions that the
//! `userdefs` feature uses for an author's definition
//! (`flm_core::stdcore::features::userdefs`):
//!
//! - the definition's element and its declaration (`UserDefinition::element_def`), with the
//!   element named under the feature's name by `element_name_under` (`shout.macro.shout`);
//! - the element's lowering rule (`expand_in_tree_rule_for`): every use of `\shout` is
//!   replaced by the template in the document's tree before lowering, on every backend;
//! - a build hook (`LateInit`) that compiles the template once the registry's language is
//!   built (`UserDefinition::compile`).
//!
//! The tests of `flm-cli` include this file too (`tests/phfpack.rs`), to run the package
//! through the library entry point `flm_cli::run` as the example does.

use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, LateInit, LateInitError, PackageEntry, PackageError,
    PackageFactory,
};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::UserDefinition;
use flm_core::lang::{BuiltLang, FlmDiagnostics};
use flm_core::stdcore::features::userdefs::{Definition, ExpandPolicy, element_name_under, expand_in_tree_rule_for};

/// The name of the package, which a document lists under `use`.
const PACKAGE: &str = "shout";

/// The name of the feature, which also begins the name of the macro's element.
const FEATURE: &str = "shout";

/// Returns the factory of the package `shout`, which uses the standard package `stdcore`:
/// the stages of its `userdefs` feature expand the uses of `\shout`.
///
/// # Errors
///
/// The error of [`PackageFactory::new`], which calls the package's build function once;
/// that function never fails.
pub fn factory() -> Result<PackageFactory, PackageError> {
    let factory = PackageFactory::new(PACKAGE, |settings: &PackageSettings| {
        let mut entry = PackageEntry::new(PACKAGE).feature(Arc::new(Shout));
        entry.features.retain(|feature| !settings.is_off(&feature.info().name));
        Ok(entry)
    })?;
    Ok(factory.uses(flm_core::stdcore::PACKAGE))
}

/// The feature `shout`: the definition of `\shout`, registered as the `userdefs` feature
/// registers an author's definition.
struct Shout;

impl FlmFeature for Shout {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Shouting").docs("The `\\shout` command.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        // `\shout{text}` in running text: one mandatory argument named `text`, written in
        // bold. Its uses are expanded in every plan, the recomposed LaTeX's included
        // (`ExpandPolicy::Always`), so the one lowering rule it needs is the rule that
        // expands it in the tree.
        let definition = Definition::macro_("shout", vec![ArgDef::mandatory("text")])
            .text("\\textbf{#{text}}")
            .expand(ExpandPolicy::Always)
            .docs("Writes its argument in bold.");
        let element = element_name_under(FEATURE, &definition);
        let rule = expand_in_tree_rule_for(element.clone(), &definition);
        let shout = UserDefinition::new(definition);
        b.element(shout.element_def(owner, element))?;
        b.lowering_rule(rule)?;
        b.late_init(Arc::new(CompileTemplate(shout)))?;
        Ok(())
    }
}

/// The build hook that compiles the template of `\shout` once the registry's language is
/// built, and reports what the compilation reports.
struct CompileTemplate(Arc<UserDefinition>);

impl LateInit for CompileTemplate {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let (Ok(reported) | Err(reported)) = self.0.compile(lang);
        for diagnostic in reported {
            diags.push(diagnostic);
        }
        Ok(())
    }
}
