//! The formulas on the LaTeX outputs: the library's own formula rules,
//! registered under names of the feature.
//!
//! On `latex-rendered` and `latex-recomposed` a formula is written as the
//! library writes it for every processor, from its nodes, for LaTeX to
//! typeset: the feature registers the four rules of
//! [`math_passthrough`](flm_ext::guest::math_passthrough) by reference, each
//! with the library's body and declaration, and writes no rule body of its
//! own. The table [`RENDERED_TABLE`](crate::math_techxt::render::RENDERED_TABLE) is not
//! read there.
//!
//! The library's declarations name these two backends as outputs that nothing
//! checks before LaTeX reads them, so the plan of each keeps one warning that
//! names the feature `math-techxt`, the rules' owner.

use flm_ext::ExtError;
use flm_ext::guest::core::lowering::DefaultMark;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::math_passthrough::{self, latex};
use flm_ext::guest::stdlib::backends::latex::{DEFINITION, MACRO, RAW, RECOMPOSED_BACKEND};
use flm_ext::guest::stdlib::features::refs::SAFE_LABELS;

/// The backend `latex-rendered`, on which LuaLaTeX typesets the formulas
/// written as `flm.pres.math-passthrough` nodes.
pub const RENDERED_BACKEND: &str = "latex-rendered";

/// The rule of `flm.math` on `latex-rendered`: the library's rule that stages
/// `flm.pres.math-passthrough`.
pub const MATH_RENDERED_RULE: &str = "math-techxt.math.lower.latex-rendered";

/// The rule of `flm.math.environment` on `latex-rendered`: the library's rule
/// that stages `flm.pres.math-passthrough`.
pub const ENVIRONMENT_RENDERED_RULE: &str = "math-techxt.math-environment.lower.latex-rendered";

/// The rule of `flm.math` on `latex-recomposed`: the library's rule of that
/// backend.
pub const MATH_RECOMPOSED_RULE: &str = "math-techxt.math.lower.latex-recomposed";

/// The rule of `flm.math.environment` on `latex-recomposed`: the library's
/// rule of that backend.
pub const ENVIRONMENT_RECOMPOSED_RULE: &str = "math-techxt.math-environment.lower.latex-recomposed";

/// Registers the four formula rules of the LaTeX outputs, with the peer
/// declarations the constructors' documentation lists: the two rules that
/// stage `flm.pres.math-passthrough`, marked for `latex-rendered`, and the
/// two rules of `latex-recomposed`, whose mark is the library's. The
/// environment rule of `latex-rendered` also reads the anchors and counters
/// tables, which [`declare_peer_tables`](super::declare_peer_tables)
/// declares.
///
/// # Errors
///
/// The host's refusal.
pub fn register_formula_rules(b: &mut FeatureBuilder) -> Result<(), ExtError> {
    b.peer_backend(RENDERED_BACKEND, "the rendered LaTeX, on which the library's rules write the formulas")?;
    b.peer_backend(RECOMPOSED_BACKEND, "the recomposed LaTeX, on which the library's rules write the formulas")?;
    b.peer_element(MACRO, "the recomposed LaTeX's macro construct, which the library's rules stage")?;
    b.peer_element(RAW, "the recomposed LaTeX's raw construct, which the library's rules stage")?;
    b.peer_element(DEFINITION, "the recomposed LaTeX's definition construct, which the library's rules stage")?;
    b.peer_table(SAFE_LABELS, "the references feature's labels spelled for LaTeX, read for the labels of equation lines")?;
    let rendered = || DefaultMark::OnBackends(vec![RENDERED_BACKEND.into()]);
    b.lowering_rule(math_passthrough::math_rule_named(MATH_RENDERED_RULE, rendered())?)?;
    b.lowering_rule(math_passthrough::environment_rule_named(ENVIRONMENT_RENDERED_RULE, rendered())?)?;
    b.lowering_rule(latex::math_rule_named(MATH_RECOMPOSED_RULE)?)?;
    b.lowering_rule(latex::environment_rule_named(ENVIRONMENT_RECOMPOSED_RULE)?)
}
