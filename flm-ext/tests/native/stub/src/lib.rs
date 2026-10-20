//! The stub extension of the engine `native`'s proof, written against the guest
//! client `flm_ext::guest`: compiled into the test program of
//! `flm-ext/tests/native.rs`, which includes this file as a module (so it uses no
//! `crate::` path), and built unchanged for `wasm32-unknown-unknown`.
//!
//! Its feature `stub` declares the table `stub.tally`, written by the stage
//! `stub.tally` (one run per tree), which reads the option `stub.tagging`, calls the
//! host service `probe` and pushes one item per tree; and the macro
//! `\stubbox{content}` (the element `stub.box`), whose lowering rule lowers the
//! content and writes it between angle brackets. While the rule lowers its
//! content, [`lowering_content`] answers `true` on the thread: the proof tests
//! read it from the other extension's code, to check that this code runs inside
//! the stub's request.

use std::cell::Cell;
use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::host::HostHandle;
use flm_ext::guest::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::guest::model::element::ElementSpec;
use flm_ext::guest::techy::node::Node;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Value, json};

/// The table the stage writes.
const TALLY: &str = "stub.tally";

/// The option the stage reads.
const TAGGING: &str = "stub.tagging";

// The static `FLM_EXTENSION` (on every target) and the module's exports (on `wasm32`).
flm_ext::guest::extension!(extension);

thread_local! {
    /// How many requests `lower_nodes` of the rule of `\stubbox` run on this thread.
    static LOWERING: Cell<u32> = const { Cell::new(0) };
}

/// Whether the rule of `\stubbox` is lowering its content on this thread: whether one of
/// its requests `lower_nodes` runs.
pub fn lowering_content() -> bool {
    LOWERING.with(|running| running.get() > 0)
}

/// The settings the feature declares: the option `stub.tagging`, set by a render.
fn settings() -> Value {
    json!({
        "options": {
            TAGGING: {
                "bind": "render",
                "docs": "How the tally is tagged.",
                "schema": {
                    "type": "object",
                    "properties": { "tag": { "type": "string", "description": "The tag of each item." } },
                },
            },
        },
    })
}

fn extension(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().uses(["stdcore"]).host(["probe"]).feature(
            "stub",
            Feature::new().title("Stub").docs("The native engine's stub.").settings(settings()).register(register),
        ),
    );
}

/// The words of `text`: its runs of letters and digits.
fn words(text: &str) -> usize {
    text.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).count()
}

/// The tally of each tree: its words, the option's tag, and what the host service
/// `probe` answers for them.
struct Tally;

impl Stage for Tally {
    fn decl(&self) -> StageDecl {
        StageDecl::new("stub.tally", Effect::Compute, Scope::PerTree).writes(TALLY).docs("tallies the words of the tree")
    }

    fn needs(&self) -> Needs {
        Needs::new().option(TAGGING)
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let tag = cx.option(TAGGING)?["tag"].as_str().unwrap_or("untagged").to_string();
        let tree = cx.tree_id()?.unwrap_or_default();
        let mut count = 0;
        if let Some(root) = cx.my_tree()? {
            for node in root.descendants()? {
                if node.is_chars()? {
                    count += words(&node.chars()?.unwrap_or_default());
                }
            }
        }
        let HostHandle::Service(probe) = cx.host("probe")? else {
            return Err(ExtError::Handler { detail: "`probe` must be a named service".to_string(), data: Value::Null });
        };
        let answer = probe.call("tally", json!({ "tree": tree, "words": count, "tag": tag }))?;
        out.table(TALLY)?.push(json!({ "tree": tree, "words": count, "tag": tag, "probe": answer }))
    }
}

/// The rule's body of `\stubbox`: the lowered content between angle brackets.
struct Boxed;

impl Lower for Boxed {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let content = node.argument_content_nodes_named("content")?.unwrap_or_default();
        let mut staged = vec![lw.text(cx, "<")?];
        LOWERING.with(|running| running.set(running.get() + 1));
        let lowered = lw.lower_nodes(cx, &content);
        LOWERING.with(|running| running.set(running.get() - 1));
        staged.extend(lowered?);
        staged.push(lw.text(cx, ">")?);
        Ok(Lowering::Emit(staged))
    }
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    b.table(TALLY, "the words of each tree, tagged")?;
    b.stage(Arc::new(Tally))?;
    let syntax = SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] };
    b.element(ElementDef::new(&owner, ["stubbox"], syntax, ElementSpec::new("stub.box")))?;
    b.lowering_rule(LoweringRule::new(
        "stub.box.lower",
        "stub.box",
        LoweringDecl::new(),
        Arc::new(Boxed),
        DefaultMark::Everywhere,
    )?)
}
