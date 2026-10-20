//! The failing guest of the WebAssembly case: the extension `failing`, whose one
//! feature `failing` declares `\fails`, whose hook answers an error; `\panics`,
//! whose hook panics (the module aborts, a trap); the stage `failing.loop`
//! (the table `failing.looped`), which never finishes; and the stage
//! `failing.grow` (the table `failing.grown`), which allocates until its memory
//! cannot grow.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::lang::mint::ArgDef;
use flm_ext::guest::lang::rewrites::userdefs::{
    Definition, Expand, ExpandCx, Expander, Expansion, UserDefinition,
};
use flm_ext::guest::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_ext::guest::techy::node::Node;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Value, json};

flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    ext.package("_", Package::new().uses(["stdcore"]).feature("failing", Feature::new().register(register)));
}

/// Declares the macro `name` of one mandatory argument, expanded by `hook`, with its
/// lowering rule.
fn declare(b: &mut FeatureBuilder, owner: &str, name: &str, hook: Arc<dyn Expand>) -> Result<(), ExtError> {
    let definition = Definition::macro_(name, vec![ArgDef::mandatory("text")]);
    let element = element_name_under("failing", &definition)?;
    b.element(UserDefinition::new(definition.clone())?.element_def_with(owner, &element, hook)?)?;
    b.lowering_rule(expand_in_tree_rule_for(&element, &definition)?)
}

/// The hook of `\fails`: answers an error of its own.
struct Fails;

impl Expand for Fails {
    fn expand(&self, _: &Node, _: &mut ExpandCx, _: &mut Expander) -> Result<Expansion, ExtError> {
        Err(ExtError::Handler { detail: "the hook is not ready".to_string(), data: json!({ "name": "NotReady" }) })
    }
}

/// The hook of `\panics`: panics.
struct Panics;

impl Expand for Panics {
    fn expand(&self, _: &Node, _: &mut ExpandCx, _: &mut Expander) -> Result<Expansion, ExtError> {
        panic!("the hook is not ready either")
    }
}

/// A stage of the document named `name`, writing the table `table`, whose run is `run`.
struct Failing {
    name: &'static str,
    table: &'static str,
    run: fn() -> Result<(), ExtError>,
}

impl Stage for Failing {
    fn decl(&self) -> StageDecl {
        StageDecl::new(self.name, Effect::Compute, Scope::Document).writes(self.table)
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, _: &StageCx, _: &mut StageOut) -> Result<(), ExtError> {
        (self.run)()
    }
}

/// Counts for ever.
fn spin() -> Result<(), ExtError> {
    let mut count: u64 = 0;
    loop {
        count = std::hint::black_box(count.wrapping_add(1));
    }
}

/// Keeps allocating a mebibyte more, until an allocation fails and the module aborts.
fn grow() -> Result<(), ExtError> {
    let mut kept: Vec<Vec<u8>> = Vec::new();
    loop {
        kept.push(std::hint::black_box(vec![1; 1 << 20]));
    }
}

fn register(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    declare(b, &owner, "fails", Arc::new(Fails))?;
    declare(b, &owner, "panics", Arc::new(Panics))?;
    b.table("failing.looped", "never written: its stage loops")?;
    b.stage(Arc::new(Failing { name: "failing.loop", table: "failing.looped", run: spin }))?;
    b.table("failing.grown", "never written: its stage grows its memory until it fails")?;
    b.stage(Arc::new(Failing { name: "failing.grow", table: "failing.grown", run: grow }))?;
    Ok(())
}
