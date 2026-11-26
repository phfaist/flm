//! A module that holds two extensions, `first` and `second`: a table of code of two rows,
//! over which the macro `extensions!` writes the module's exports. Each extension declares
//! the package `_` with one feature named as the extension, so that the feature a package
//! holds shows which row the entry `define` ran. Each also declares the package `shared`
//! with the feature `stage`, the same names in both, whose `register` records the stage
//! `pair.mark`; the stage writes a note naming its extension, so that the note shows whose
//! stage ran when both extensions are defined in one instance.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::{CompiledExtension, Extension, Feature, NamedExtension, Package};
use serde_json::Value;

/// The extension `first`.
static FIRST: CompiledExtension = CompiledExtension::new(first, "first", env!("CARGO_PKG_VERSION"));

/// The extension `second`.
static SECOND: CompiledExtension = CompiledExtension::new(second, "second", env!("CARGO_PKG_VERSION"));

/// The module's table of code.
pub static COMPILED_EXTENSIONS: &[NamedExtension] =
    &[NamedExtension { stem: "first", compiled: &FIRST }, NamedExtension { stem: "second", compiled: &SECOND }];

flm_ext::guest::extensions!(COMPILED_EXTENSIONS);

fn first(ext: &mut Extension) {
    ext.package("_", Package::new().feature("first", Feature::new().title("First").register(register)));
    ext.package("shared", Package::new().feature("stage", Feature::new().register(|b, _| b.stage(Arc::new(Mark("first"))))));
}

fn second(ext: &mut Extension) {
    ext.package("_", Package::new().feature("second", Feature::new().title("Second").register(register)));
    ext.package("shared", Package::new().feature("stage", Feature::new().register(|b, _| b.stage(Arc::new(Mark("second"))))));
}

/// The `register` of the features of the package `_`: declares nothing.
fn register(_: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    Ok(())
}

/// The stage `pair.mark` of the extension it names: writes the note `pair.mark` that names
/// the extension.
struct Mark(&'static str);

impl Stage for Mark {
    fn decl(&self) -> StageDecl {
        StageDecl::new("pair.mark", Effect::Compute, Scope::Document)
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, _: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        out.diags(Diagnostic::note("pair.mark", format!("the stage of the extension ‘{}’", self.0)))
    }
}
