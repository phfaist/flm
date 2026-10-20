//! The extension `two` of the program of `flm-ext/tests/native/program/`, written against the
//! guest client: its feature `two` declares the table `two.notes`.

use flm_ext::ExtError;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::Value;

flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    ext.package("_", Package::new().uses(["stdcore"]).feature("two", Feature::new().title("Two").register(register)));
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    b.table("two.notes", "notes of the extension `two`")
}
