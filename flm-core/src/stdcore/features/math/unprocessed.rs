//! The rule of last resort of the display-math environments: a display-math
//! environment that no feature processing mathematics lowers is an Error.
//!
//! A display-math environment ([`ENVIRONMENT`], `flm.math.environment`) is
//! lowered by the rules of the feature that processes mathematics (the
//! module documentation of [`math`](super), *Processing mathematics*). Where
//! no such feature has a rule for the environment on the backend at hand, in
//! a registry without a processing feature for instance, [`RULE`] lowers it,
//! as the `lowering` feature's rule of last resort
//! ([`MathGroupRule`](crate::stdcore::backends::lowering::MathGroupRule))
//! lowers a math group: it reports [`MathUnprocessed`]
//! (`flm.math.unprocessed`, an Error) at the environment, and lowers the
//! content of the environment's equation lines through the rules as ordinary
//! content, inside a `flm.pres.span` of role `flm.math` and kind `display`. A
//! character run is text, an unknown command its `flm.pres.unknown`, a text
//! command its `flm.pres.math-text`. The `\\`, `\label`, `\tag` and
//! `\nonumber` of the body are data of the lines ([`lines`](super::lines)) and
//! are not written.
//!
//! The rule is registered on every backend and marked [`DefaultMark::Never`],
//! so it is chosen on a backend only where it is the sole candidate for the
//! environment: the rule of a processing feature applies wherever one is
//! registered for the backend. With it, every backend renders every
//! display-math environment, so the plan of a standard workflow builds for a
//! registry that holds the `math` feature and no processing feature.

// See decision rationale at [§flm-dd-dr:math-processor-demands-content].

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::error::Diagnostic;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::lang::mint::FIELD;
use crate::lang::structural::MATH;
use crate::model::element::element_of;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::rules::MATH_DISPLAY_KIND;
use crate::stdcore::backends::lowering::MathUnprocessed;
use crate::stdcore::backends::pres::{self, SpanPayload};

use super::ENVIRONMENT;

/// The name of the rule of last resort of [`ENVIRONMENT`] on every backend,
/// `flm.math.environment.lower.default`, whose body is
/// [`UnprocessedEnvironmentRule`].
pub const RULE: &str = "flm.math.environment.lower.default";

/// The body of [`RULE`]: it reports [`MathUnprocessed`]
/// (`flm.math.unprocessed`, an Error) at the environment's span, and stages a
/// `flm.pres.span` of role `flm.math` and kind `display` over the content of
/// the environment's equation lines, lowered
/// through the rules as ordinary content. The fields of the body (`\\`,
/// `\label`, `\tag`, `\nonumber`) are not lowered. See the module
/// documentation.
///
/// It returns a [`LowerError`] when a node of the content could not be
/// lowered or a node could not be staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnprocessedEnvironmentRule;

impl Lower for UnprocessedEnvironmentRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.diagnose(Diagnostic::error(MathUnprocessed, node.span().clone()));
        let mut content = Vec::new();
        if let Some(body) = node.body() {
            let start = body.range().start;
            for (index, child) in (0u32..).zip(body.iter()) {
                if element_of(&child).name == FIELD {
                    continue;
                }
                let one = node
                    .tree()
                    .slice(start + index..start + index + 1)
                    .ok_or_else(|| LowerError::structure("a child outside its parent's body"))?;
                content.extend(lw.lower_nodes(cx, one)?);
            }
        }
        let payload = SpanPayload {
            role: Some(Name::from(MATH)),
            kinds: vec![Name::from(MATH_DISPLAY_KIND)],
            ..SpanPayload::default()
        };
        let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, content)?;
        Ok(Lowering::Emit(vec![span]))
    }
}

/// The rule [`RULE`]: [`UnprocessedEnvironmentRule`] for [`ENVIRONMENT`] on
/// every backend, never a default.
fn rule() -> LoweringRule {
    LoweringRule::new(
        RULE,
        ENVIRONMENT,
        LoweringDecl::new().produces(pres::SPAN),
        Arc::new(UnprocessedEnvironmentRule),
        // The rule of last resort: never a default, so that the rule of a
        // feature that processes mathematics applies wherever one is
        // registered for the backend.
        DefaultMark::Never,
    )
    .docs(
        "a display-math environment that no feature processes is an Error, and lowers to a \
         `flm.pres.span` of role `flm.math` over the content of its equation lines lowered as \
         ordinary content",
    )
}

/// Registers [`RULE`], as part of the `math` feature.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.lowering_rule(rule())?;
    Ok(())
}

