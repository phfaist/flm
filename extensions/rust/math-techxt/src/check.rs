//! The check stage `math-techxt.check` and the feature's capability of the
//! same name.
//!
//! The stage ([`Check`]) is the feature's check of the formulas: it provides
//! `flm.math.content` and the capability [`CHECK`], which the feature adds to
//! the collection `flm.validated`, so that it runs on every workflow that
//! validates a document. It asks one thing of the display-math environments:
//! every equation line holds mathematics. A line made only of blank
//! characters, comments, `\label`, `\tag` and `\nonumber` is reported as
//! [`EMPTY_LINE`], an Error, at the line's first node, or at the environment
//! when the line holds none.
//!
//! techxt does not run here. The stage that renders the formulas with techxt
//! (`math-techxt.render`) is in a plan only where a selected rule reads its
//! table, which the rules of the text and HTML outputs do; what techxt reports
//! about a formula is therefore reported on those runs only.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::stdlib::features::math::{CONTENT, ENVIRONMENT, equation_lines};
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::Node;
use serde_json::json;

/// The feature's capability `math-techxt.check`, also the name of the stage
/// that provides it: the feature adds it to the collection `flm.validated`, so
/// that every workflow that validates a document checks its formulas.
pub const CHECK: &str = "math-techxt.check";

/// The diagnostic `math-techxt.empty-line` (an Error): an equation line of a
/// display-math environment holds no mathematics.
pub const EMPTY_LINE: &str = "math-techxt.empty-line";

/// Declares the capability [`CHECK`], adds it to the collection
/// `flm.validated`, and declares the stage.
///
/// # Errors
///
/// The host's refusal.
pub fn register_check(b: &mut FeatureBuilder) -> Result<(), ExtError> {
    b.capability(CHECK, "The mathematics of the document has been checked by the feature `math-techxt`.", None)?;
    b.collection_member("flm.validated", CHECK)?;
    b.stage(Arc::new(Check))
}

/// Whether a node of an equation line's content is mathematics: anything but
/// blank characters and comments.
///
/// # Errors
///
/// The host's refusal.
pub fn is_mathematics(node: &Node) -> Result<bool, ExtError> {
    if node.is_comment()? {
        return Ok(false);
    }
    Ok(!(node.is_chars()? && node.chars()?.unwrap_or_default().trim().is_empty()))
}

/// The stage [`CHECK`]. See the module documentation.
struct Check;

impl Stage for Check {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK, Effect::Check, Scope::PerTree).provides(CONTENT).provides(CHECK).docs(
            "The check of the formulas rendered by techxt: it reports every equation line that holds no \
             mathematics, and provides `flm.math.content` and `math-techxt.check`, a member of the collection \
             `flm.validated`.",
        )
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let Some(tree) = cx.my_tree()? else { return Ok(()) };
        for node in tree.root()?.descendants()?.elements([ENVIRONMENT])? {
            let node = node?;
            let name = node.name()?.unwrap_or_default();
            for (index, line) in equation_lines(&node)?.iter().enumerate() {
                let mut mathematics = false;
                let content_nodes = line.content.to_vec()?;
                for content in &content_nodes {
                    if is_mathematics(content)? {
                        mathematics = true;
                        break;
                    }
                }
                if mathematics {
                    continue;
                }
                out.diags(
                    Diagnostic::error(
                        EMPTY_LINE,
                        format!(
                            "Line {index} (counting from 0) of the ‘{name}’ environment must hold mathematics; a \
                             ‘\\label’, ‘\\tag’ or ‘\\nonumber’ alone is not mathematics"
                        ),
                    )
                    .with_data(json!({ "environment": name, "line": index }))
                    .at(content_nodes.first().unwrap_or(&node)),
                )?;
            }
        }
        Ok(())
    }
}
