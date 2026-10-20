//! Mirrors [`flm_core::stdcore::features::userdefs`]: the helpers of the
//! user-definitions feature an extension declares definitions with.

use flm_core::lang::Flm;
use flm_core::techy::core::node::NodeRef;

use crate::api::core::lowering::{LowerCx, Lowerer, Staged};
use crate::api::lang::rewrites::userdefs::Definition;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::sealed::Token;

pub use crate::api::core::lowering::LoweringRule;

/// Mirrors [`element_name_under`](flm_core::stdcore::features::userdefs::element_name_under):
/// the element name of `definition` under the prefix `prefix`. What differs:
/// the name as text.
pub fn element_name_under(prefix: &str, definition: &Definition) -> String {
    flm_core::stdcore::features::userdefs::element_name_under(prefix, definition).to_string()
}

/// Mirrors [`expand_in_tree_rule_for`](flm_core::stdcore::features::userdefs::expand_in_tree_rule_for):
/// the lowering rule that has the element `element` of `definition` expanded
/// in the tree before lowering. What differs: the rule is the extension API's
/// [`LoweringRule`], which holds the rule `flm-core` built.
pub fn expand_in_tree_rule_for(element: &str, definition: &Definition) -> LoweringRule {
    LoweringRule::from_core(flm_core::stdcore::features::userdefs::expand_in_tree_rule_for(element, definition))
}

/// Mirrors [`write_math_run`](flm_core::stdcore::features::userdefs::kept::write_math_run):
/// writes a run of nodes in mathematics mode for the recomposed LaTeX, the
/// mathematics writer of the `userdefs` feature (the source byte for byte,
/// a use of a user definition written as its call or its expansion, a text
/// command and a comment lowered through the rules), the content writer of a
/// processing feature's rules on that backend.
///
/// What differs: the parts the writer wrote are staged and answered as the
/// call's own staged nodes, each text part as a raw LaTeX node, as
/// [`stage_math_run`](flm_core::stdcore::features::userdefs::kept::stage_math_run)
/// does: a script cannot stage the raw construct itself, whose payload is not
/// among the presentation payloads given as data. The calling rule declares
/// that it produces the raw construct and the definition construct
/// (`flm.pres.latex.raw`, `flm.pres.latex.definition`), with peer
/// declarations of them.
///
/// # Errors
///
/// `cx` is not the call's context ([`ExtError::Argument`]), or `flm-core`
/// refuses a node ([`ExtError::Core`]).
pub fn write_math_run<'t>(
    cx: &mut dyn LowerCx,
    lw: &mut dyn Lowerer,
    nodes: &[Box<dyn Node<'t> + 't>],
) -> Result<Vec<Staged>, ExtError> {
    const METHOD: &str = "write_math_run";
    let nodes: Vec<NodeRef<'_, Flm>> = nodes.iter().map(|node| node.core_node_ref(Token::new())).collect();
    lw.stage_core(Token::new(), METHOD, cx, &mut |cx, lw| {
        flm_core::stdcore::features::userdefs::kept::stage_math_run(cx, lw, &nodes)
    })
}
