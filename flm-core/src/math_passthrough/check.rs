//! The check stage of the [`math-passthrough`](super) feature,
//! `flm.math-passthrough.check`: the provider of the capability
//! [`flm.math.content`](crate::stdcore::features::math::CONTENT), which the
//! feature adds to the collection [`flm.validated`](crate::stdcore::VALIDATED),
//! so that every workflow that validates a document runs this check.
//!
//! Passthrough mathematics reads nothing inside a formula, so the check
//! accepts every formula as written. It asks one thing of the display-math
//! environments: every equation line holds mathematics. A line made only of
//! whitespace, comments, `\label`, `\tag` and `\nonumber` (an empty body, a
//! `\\` at the start of a body, two `\\` in a row, a `\label` after the last
//! `\\`) is reported as [`EmptyLine`] (`flm.math-passthrough.empty-line`), an
//! Error. The line is written as written: the rules write every line the
//! `math` feature reads ([`equation_lines`](crate::stdcore::features::math::lines::equation_lines)),
//! an empty one included.

use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::data::store::StageOut;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::model::element::{element_of, element_value};
use crate::stdcore::features::math::{CONTENT, ENVIRONMENT, MathEnvironment, lines};

/// The name of the check stage, `flm.math-passthrough.check`: it provides
/// [`flm.math.content`](CONTENT), a member of the collection
/// [`flm.validated`](crate::stdcore::VALIDATED) that the feature adds, accepts
/// every formula, and reports every equation line that holds no mathematics
/// ([`EmptyLine`]). See the module documentation.
pub const CHECK: &str = "flm.math-passthrough.check";

/// Condition `flm.math-passthrough.empty-line` (an Error): an equation line of
/// a display-math environment holds no mathematics. Whitespace, comments,
/// `\label`, `\tag` and `\nonumber` are not mathematics, so a line made only
/// of them is empty, and so is an entirely empty body.
///
/// The check stage ([`CHECK`]) reports it at the commands the line holds, or
/// at the `\\` that ends the line when it holds none, or at the body when the
/// body is entirely empty. The recovery is to write the line as written.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.math-passthrough.empty-line",
    message = "line {line} (counting from 0) of the `{environment}` environment must hold \
               mathematics; a `\\label`, `\\tag` or `\\nonumber` alone is not mathematics"
)]
pub struct EmptyLine {
    /// The environment's name as written (`align`).
    pub environment: String,
    /// The index of the empty line among the environment's lines, counting from
    /// zero: its position in the list
    /// [`equation_lines`](crate::stdcore::features::math::lines::equation_lines)
    /// returns.
    pub line: usize,
}

/// The check stage [`CHECK`], the provider of [`CONTENT`]. See the module
/// documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct PassthroughCheck;

impl Stage for PassthroughCheck {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK, Effect::Check, Scope::PerTree).provides(CONTENT).docs(
            "The check of passthrough mathematics: it accepts every formula as written, \
             reports every equation line that holds no mathematics as \
             `flm.math-passthrough.empty-line`, and provides `flm.math.content`.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        // A tree whose rewrite failed is absent; its one diagnostic stands.
        let Some(tree) = cx.my_tree() else { return };
        for node in tree.root().descendants() {
            if element_of(&node).name != ENVIRONMENT {
                continue;
            }
            let Some(value) = element_value::<MathEnvironment>(&node) else { continue };
            for (line, span) in lines::empty_lines(&node) {
                let condition = EmptyLine { environment: value.name.to_string(), line };
                out.diags.push(Diagnostic::error(condition, span));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_is_a_per_tree_check_providing_content() {
        let decl = PassthroughCheck.decl();
        assert_eq!(decl.name, CHECK);
        assert_eq!(decl.effect, Effect::Check);
        assert_eq!(decl.scope, Scope::PerTree);
        assert_eq!(decl.provides, [crate::model::names::CapName::from(CONTENT)]);
        assert!(decl.writes.is_empty());
    }
}
