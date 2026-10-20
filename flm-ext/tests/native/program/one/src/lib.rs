//! The extension `one` of the program of `flm-ext/tests/native/program/`, written against the
//! guest client: its feature `one` declares the table `one.notes`.

use flm_ext::ExtError;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::Value;

flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    ext.package("_", Package::new().uses(["stdcore"]).feature("one", Feature::new().title("One").register(register)));
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    b.table("one.notes", "notes of the extension `one`")
}
