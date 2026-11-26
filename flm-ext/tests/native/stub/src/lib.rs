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
//!
//! The feature also declares the table `stub.tex`, written by the stage `stub.tex`
//! (one run per tree), which calls the writer of a formula's TeX through the guest
//! client: per math group of the tree its parts (`group_tex`), per display-math
//! environment the parts and the trailing run of each line (`line_tex`, over the
//! content of each line `equation_lines` answers), each hole read back as its
//! command and its node's source.
//!
//! The feature builds the four formula rules of `math-passthrough` through the guest
//! client under names of its own (`stub.math.…`): it registers the two that stage
//! `flm.pres.math-passthrough`, marked never a default, with the peer tables the
//! environment's rule reads, so that the formulas stay `math-passthrough`'s; it builds
//! the two of the recomposed LaTeX without registering them, since their mark is the
//! library's (the default on that backend), which `math-passthrough`'s own rules hold.

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
use flm_ext::guest::lang::rewrites::userdefs::StagedNodes;
use flm_ext::guest::math_passthrough::{self, latex};
use flm_ext::guest::model::element::ElementSpec;
use flm_ext::guest::stdlib::features::math::{self, TexPart};
use flm_ext::guest::techy::node::{Node, NodeKindName};
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Value, json};

/// The table the stage writes.
const TALLY: &str = "stub.tally";

/// The option the stage reads.
const TAGGING: &str = "stub.tagging";

/// The table of the formulas' TeX.
const TEX: &str = "stub.tex";

/// The name of `math-passthrough`'s rule of `flm.math`, registered by the feature.
pub const MATH_RULE: &str = "stub.math.lower";

/// The name of `math-passthrough`'s rule of `flm.math.environment`, registered by the
/// feature.
pub const ENVIRONMENT_RULE: &str = "stub.math.environment.lower";

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
        Package::new().host(["probe"]).feature(
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
        if let Some(my_tree) = cx.my_tree()? {
            for node in my_tree.root()?.descendants()?.kinds(&[NodeKindName::Chars])? {
                count += words(&node?.chars()?.unwrap_or_default());
            }
        }
        let HostHandle::Service(probe) = cx.host("probe")? else {
            return Err(ExtError::Handler { detail: "`probe` must be a named service".to_string(), data: Value::Null });
        };
        let answer = probe.call("tally", json!({ "tree": tree, "words": count, "tag": tag }))?;
        out.table(TALLY)?.push(json!({ "tree": tree, "words": count, "tag": tag, "probe": answer }))
    }
}

/// The parts `parts` as data: `{"tex": text}`, or `{"hole": {"command", "source"}}`
/// with the source of the hole's node.
fn parts_value(parts: &[TexPart]) -> Result<Value, ExtError> {
    let mut values = Vec::with_capacity(parts.len());
    for part in parts {
        values.push(match part {
            TexPart::Tex(text) => json!({ "tex": text }),
            TexPart::Hole(hole) => {
                json!({ "hole": { "command": hole.command, "source": hole.node.span_content()? } })
            }
        });
    }
    Ok(Value::Array(values))
}

/// The TeX of each formula of the tree, as the writer answers it: one item per math
/// group (`{"group": parts}`) and one per display-math environment (`{"lines":
/// [{"parts", "trailing"}, …]}`), in the order of the tree.
struct Tex;

impl Stage for Tex {
    fn decl(&self) -> StageDecl {
        StageDecl::new("stub.tex", Effect::Compute, Scope::PerTree).writes(TEX).docs("writes the TeX of the formulas")
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let Some(my_tree) = cx.my_tree()? else { return Ok(()) };
        for node in my_tree.root()?.descendants()? {
            let node = node?;
            let element = node.element_name()?;
            if element == math::MATH {
                out.table(TEX)?.push(json!({ "group": parts_value(&math::group_tex(&node)?)? }))?;
            } else if element == math::ENVIRONMENT {
                let mut lines = Vec::new();
                let mut after_line_end = false;
                for line in math::equation_lines(&node)? {
                    let line = math::line_tex(&line.content, after_line_end)?;
                    after_line_end = line.ends_line();
                    lines.push(json!({ "parts": parts_value(&line.parts)?, "trailing": line.trailing }));
                }
                out.table(TEX)?.push(json!({ "lines": lines }))?;
            }
        }
        Ok(())
    }
}

/// The rule's body of `\stubbox`: the lowered content between angle brackets.
struct Boxed;

impl Lower for Boxed {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let content = node.argument_content_nodes_named("content")?.unwrap_or_default();
        let mut staged = StagedNodes::default();
        staged.push(lw.text(cx, "<")?);
        LOWERING.with(|running| running.set(running.get() + 1));
        let lowered = lw.lower_nodes(cx, &content);
        LOWERING.with(|running| running.set(running.get() - 1));
        staged.push_all(&lowered?);
        staged.push(lw.text(cx, ">")?);
        Ok(Lowering::Emit(staged))
    }
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    b.table(TALLY, "the words of each tree, tagged")?;
    b.stage(Arc::new(Tally))?;
    b.table(TEX, "the TeX of the formulas of each tree")?;
    b.stage(Arc::new(Tex))?;
    let syntax = SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] };
    b.element(ElementDef::new(&owner, ["stubbox"], syntax, ElementSpec::new("stub.box")))?;
    b.lowering_rule(LoweringRule::new(
        "stub.box.lower",
        "stub.box",
        LoweringDecl::new(),
        Arc::new(Boxed),
        DefaultMark::Everywhere,
    )?)?;
    b.peer_table(math::ANCHORS, "the anchors table, which the environment's rule reads")?;
    b.peer_table(math::COUNTERS, "the counter table, which the environment's rule reads")?;
    b.lowering_rule(math_passthrough::math_rule_named(MATH_RULE, DefaultMark::Never)?)?;
    b.lowering_rule(math_passthrough::environment_rule_named(ENVIRONMENT_RULE, DefaultMark::Never)?)?;
    latex::math_rule_named("stub.math.lower.latex")?;
    latex::environment_rule_named("stub.math.environment.lower.latex")?;
    Ok(())
}
