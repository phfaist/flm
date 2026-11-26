//! A collection of two extensions, `keep` and `show`: a table of code of two rows, over
//! which the macro `extensions!` writes the module's exports, and the manifest of each
//! recorded in the module's section `flmextensions`. The extension `keep` sets a value that
//! the module keeps in its memory (the stage `keep.write`, the table `keep.kept`), traps
//! (the stage `keep.trap`, the table `keep.trapped`), and allocates until its memory cannot
//! grow (the stage `keep.grow`, the table `keep.grown`); its manifest sets a memory limit of
//! 8 MiB. The extension `show` writes a note that shows the value the module keeps (the
//! stage `show.read`, the table `show.shown`); its manifest sets no limit.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::{CompiledExtension, Extension, Feature, NamedExtension, Package};
use serde_json::Value;

flm_ext::collection::record!("rust", "keep", "flmextension.yaml", "../keep/flmextension.yaml");
flm_ext::collection::record!("rust", "show", "flmextension.yaml", "../show/flmextension.yaml");

/// The extension `keep`.
static KEEP: CompiledExtension = CompiledExtension::new(keep, "keep", env!("CARGO_PKG_VERSION"));

/// The extension `show`.
static SHOW: CompiledExtension = CompiledExtension::new(show, "show", env!("CARGO_PKG_VERSION"));

/// The module's table of code.
pub static COMPILED_EXTENSIONS: &[NamedExtension] =
    &[NamedExtension { stem: "keep", compiled: &KEEP }, NamedExtension { stem: "show", compiled: &SHOW }];

flm_ext::guest::extensions!(COMPILED_EXTENSIONS);

/// The value the module keeps in its memory, which `keep.write` sets and `show.read` shows.
static KEPT: AtomicU32 = AtomicU32::new(0);

fn keep(ext: &mut Extension) {
    ext.package("_", Package::new().feature("keep", Feature::new().register(register_keep)));
}

fn show(ext: &mut Extension) {
    ext.package("_", Package::new().feature("show", Feature::new().register(register_show)));
}

/// A stage of the document named `name`, writing the table `table`, whose run is `run`.
struct Run {
    name: &'static str,
    table: &'static str,
    run: fn(&mut StageOut) -> Result<(), ExtError>,
}

impl Stage for Run {
    fn decl(&self) -> StageDecl {
        StageDecl::new(self.name, Effect::Compute, Scope::Document).writes(self.table)
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, _: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        (self.run)(out)
    }
}

/// Sets the value the module keeps.
fn write(_: &mut StageOut) -> Result<(), ExtError> {
    KEPT.store(42, Ordering::Relaxed);
    Ok(())
}

/// Panics: the module aborts, a trap.
fn trap(_: &mut StageOut) -> Result<(), ExtError> {
    panic!("the stage traps")
}

/// Keeps allocating a mebibyte more, until an allocation fails and the module aborts.
fn grow(_: &mut StageOut) -> Result<(), ExtError> {
    let mut kept: Vec<Vec<u8>> = Vec::new();
    loop {
        kept.push(std::hint::black_box(vec![1; 1 << 20]));
    }
}

/// Writes the note `show.value`, which shows the value the module keeps.
fn read(out: &mut StageOut) -> Result<(), ExtError> {
    out.diags(Diagnostic::note("show.value", format!("the value kept is ‘{}’", KEPT.load(Ordering::Relaxed))))
}

fn register_keep(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    b.table("keep.kept", "never written: its stage sets the value the module keeps")?;
    b.stage(Arc::new(Run { name: "keep.write", table: "keep.kept", run: write }))?;
    b.table("keep.trapped", "never written: its stage traps")?;
    b.stage(Arc::new(Run { name: "keep.trap", table: "keep.trapped", run: trap }))?;
    b.table("keep.grown", "never written: its stage grows its memory until it fails")?;
    b.stage(Arc::new(Run { name: "keep.grow", table: "keep.grown", run: grow }))
}

fn register_show(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    b.table("show.shown", "never written: its stage writes a note that shows the value the module keeps")?;
    b.stage(Arc::new(Run { name: "show.read", table: "show.shown", run: read }))
}
