//! `\ensuremath{…}`: mathematics wherever it is written.
//!
//! LaTeX's `\ensuremath{X}` typesets `X` as mathematics: in text it is an
//! inline formula, and inside a formula it is `X` itself. The `math` feature
//! declares the command in both parsing modes, with one element type,
//! [`ENSUREMATH`] (`flm.math.ensuremath`), and one default rule on every
//! backend, [`RULE`].
//!
//! # In text: an inline formula
//!
//! In text mode, `\ensuremath{X}` is read as the inline formula `\(X\)`. Its
//! one argument must be written in braces, and the braces delimit a **math
//! group**: a node of the structural element
//! [`flm.math`](crate::lang::structural::MATH), inline
//! ([`MathGroupForm::Inline`]), whose content `X` is parsed in math mode, as
//! the content of `\(…\)` is. The group is the argument's content
//! ([`argument_content_nodes`](techy::core::node::NodeRef::argument_content_nodes)
//! answers it alone); the `\ensuremath` node around it holds nothing else.
//!
//! A feature that processes mathematics therefore meets this formula as it
//! meets `\(X\)`: the same element, the same form (not displayed, so no
//! number), the same content, with its text commands. Its rule for `flm.math`
//! lowers the group, reached through [`RULE`], which lowers the argument; its
//! stages find the group among the tree's descendants. The one difference is
//! the delimiters written in the source: the group's delimiters are `{` and
//! `}` (what [`group_delimiters`](techy::core::node::NodeRef::group_delimiters)
//! answers), so an output that writes a formula's delimiters takes them from
//! the group's form, as every output of the standard backends does, not from
//! the source.
//!
//! The recomposed LaTeX writes this formula as written, `\ensuremath{X}`:
//! [`group_opening`](super::latex::group_opening) and
//! [`group_closing`](super::latex::group_closing), which a processing
//! feature's rule for `flm.math` calls on that backend, write `\ensuremath{`
//! and `}` for it in place of `\(` and `\)`. The command keeps the formula
//! valid LaTeX where it ends up inside another formula: a kept user
//! definition whose template writes `\ensuremath`, used inside a formula, or
//! an argument that a template places in mathematics.
//!
//! Without braces (`\ensuremath x`) the argument is missing, an Error.
//!
//! # In mathematics: the argument in its place
//!
//! In math mode, `\ensuremath{X}` stands for `X` as if `X` were written in its
//! place. The command takes one mandatory argument, read in math mode like the
//! formula around it. [`RULE`] lowers the argument's content, so a formula
//! lowered node by node writes `X`. A writer that writes a formula's TeX from
//! its nodes for a renderer writes the argument's content and leaves out the
//! command: the source walk of
//! [`math-passthrough`](crate::math_passthrough), whose TeX reaches the HTML
//! page's MathJax, does, and so does the `math-mathjax` extension. The
//! recomposed LaTeX writes a formula as the author wrote it, so it keeps the
//! command, which LaTeX defines, as it does in text.
//!
//! # The element and its rule
//!
//! The two declarations share the element type [`ENSUREMATH`]; the parsing
//! mode of the node tells them apart. [`RULE`]
//! (`flm.math.ensuremath.lower.default`) is the default rule on every backend:
//! it lowers the argument's content through the rules, the inline formula in
//! text mode, `X` in math mode.

// Why `\ensuremath` is the math feature's, and why its text form is a math
// group: see decision rationale at [§flm-dd-dr:feat-phfqit-output-needs].

use std::sync::Arc;

use techy::core::constructs::{
    scan_argument_noise, stage_pre_space, ArgumentParser, ChildStateSpec, ConstructParserResult,
    GroupArgumentParser, ParseContext, ParsedArgumentNodes,
};
use techy::core::node::{ContentNodes, NodeRef};
use techy::core::specs::ArgumentSpec;
use techy::core::token::{GroupRule, TokenEdge, TokenKind};
use techy::latexlike::{MathGroupForm, Mode};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::mint::{ArgDef, ArgStdKind, DefinitionShape, ElementDef, FlmArgumentParser, SyntaxDef};
use crate::lang::{Flm, FlmGroupType};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::FeatureName;
use crate::stdcore::backends::lowering::lower_arg_or_nothing;

/// The command's name, `ensuremath`, written without the leading backslash.
pub const COMMAND: &str = "ensuremath";

/// The name of the element type of `\ensuremath`, `flm.math.ensuremath`, in
/// both parsing modes.
pub const ENSUREMATH: &str = "flm.math.ensuremath";

/// The name of the default lowering rule of [`ENSUREMATH`] on every backend,
/// `flm.math.ensuremath.lower.default`, whose body is [`EnsureMathRule`].
pub const RULE: &str = "flm.math.ensuremath.lower.default";

/// The name of the command's only argument, `formula` in text mode and in
/// math mode alike.
pub const FORMULA_ARG: &str = "formula";

/// The argument parser of `\ensuremath` in text mode: a `{…}` group read as an
/// inline math group.
///
/// The group opens at a `{` of the content class and closes at its matching
/// `}`. It is staged as a group of the class
/// [`FlmGroupType::Math`] ([`MathGroupForm::Inline`]), so its content is
/// parsed in math mode, and the group itself is the argument's content. When
/// no `{` follows the command (after whitespace and comments), the argument is
/// missing and reported as a mandatory argument is.
#[derive(Debug)]
struct FormulaArgument {
    /// The rule of the math group: `{` and `}`, the inline math class.
    rule: Arc<GroupRule<Flm>>,
    /// The parser that reports a missing argument when no `{` follows.
    missing: GroupArgumentParser<Flm>,
}

impl FormulaArgument {
    fn new() -> FormulaArgument {
        FormulaArgument {
            rule: Arc::new(GroupRule {
                group_type: FlmGroupType::Math(MathGroupForm::Inline),
                open: String::from("{"),
                close: String::from("}"),
            }),
            missing: GroupArgumentParser::new(FlmGroupType::Content).with_expression_fallback(false),
        }
    }
}

impl ArgumentParser<Flm> for FormulaArgument {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let noise = scan_argument_noise(cx)?;
        let open = noise.next.clone().filter(|token| {
            matches!(cx.tokens.token_kind(token), TokenKind::GroupOpen { rule, .. }
                if rule.group_type == FlmGroupType::Content && rule.open == self.rule.open)
        });
        let Some(open) = open else {
            // No brace group: the standard parser of a braced argument reads
            // the same position and reports the missing argument.
            noise.rewind(cx);
            return self.missing.parse_argument(cx, spec);
        };
        let mut nodes = noise.nodes;
        stage_pre_space(cx, &mut nodes, &open)?;
        cx.tokens.move_to(&open, TokenEdge::EndPastPostSpace);
        let base = Arc::clone(&cx.state);
        let (group, _) =
            cx.parse_group(base, &open, Arc::clone(&self.rule), ChildStateSpec::inherit(), None)?;
        let index = u32::try_from(nodes.len()).unwrap_or(u32::MAX);
        nodes.push(group);
        Ok(Some(ParsedArgumentNodes::new(
            nodes,
            ContentNodes::InRegion(index..index.saturating_add(1)),
            (),
        )))
    }

    /// Returns `false`: the argument is mandatory.
    fn can_match_empty(&self) -> bool {
        false
    }
}

impl FlmArgumentParser for FormulaArgument {
    fn identity(&self) -> Vec<u8> {
        b"flm.math.ensuremath.formula".to_vec()
    }

    /// No user definition can declare an argument read by this parser.
    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::NoShape { token: "formula" }
    }

    fn description(&self) -> String {
        String::from("a `{…}` group read as an inline formula")
    }
}

/// The element value of `\ensuremath` in the parsing mode `mode`, inline (the
/// default block role).
fn element(mode: Mode) -> Arc<ElementSpec> {
    let docs = match mode {
        Mode::Text => "`\\ensuremath{…}` in text: an inline formula, as `\\(…\\)`.",
        _ => "`\\ensuremath{…}` inside mathematics: its argument, as if written in its place.",
    };
    Arc::new(
        ElementSpec::new(ENSUREMATH)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

/// The declaration of `\ensuremath` in text mode: a macro whose one argument
/// is an inline formula ([`FormulaArgument`]).
fn text_def(owner: &FeatureName) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [COMMAND],
        SyntaxDef::Macro {
            args: vec![ArgDef::named(
                FORMULA_ARG,
                Arc::new(FormulaArgument::new()) as Arc<dyn FlmArgumentParser>,
            )
            .docs("The formula, in braces: its content is mathematics.")],
        },
        element(Mode::Text),
    )
    .modes([Mode::Text])
    .docs(
        "`\\ensuremath{formula}`: an inline formula, as `\\(formula\\)`. The braces are \
         required; their content is read in math mode.",
    )
}

/// The declaration of `\ensuremath` in math mode: a macro with one mandatory
/// argument, read in math mode.
fn math_def(owner: &FeatureName) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [COMMAND],
        SyntaxDef::Macro {
            args: vec![ArgDef::named(FORMULA_ARG, ArgStdKind::Mandatory)
                .docs("The mathematics, as if written in place of the command.")],
        },
        element(Mode::Math),
    )
    .modes([Mode::Math])
    .docs("`\\ensuremath{formula}` inside mathematics: the formula, as if written in its place.")
}

/// The body of [`RULE`]: it lowers the argument's content through the rules,
/// the inline formula in text mode and the mathematics in math mode, and
/// stages nothing of its own.
///
/// When the argument is missing (a parse recovery left it absent and the
/// parse reported it), the rule produces nothing. It returns a [`LowerError`]
/// when the argument's content could not be lowered.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnsureMathRule;

impl Lower for EnsureMathRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(lower_arg_or_nothing(cx, lw, &node, 0)?))
    }
}

/// Registers `\ensuremath` in both parsing modes and its default lowering
/// rule, as part of the `math` feature.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let owner: FeatureName = b.feature().clone();
    b.element(text_def(&owner))?;
    b.element(math_def(&owner))?;
    b.lowering_rule(
        LoweringRule::new(
            RULE,
            ENSUREMATH,
            LoweringDecl::new(),
            Arc::new(EnsureMathRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "`\\ensuremath` lowers to its argument lowered through the rules: in text an inline \
             formula, inside mathematics the mathematics it holds",
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_declared_once_per_mode_with_one_argument() {
        let owner = FeatureName::from("math");
        let text = text_def(&owner);
        assert_eq!(text.modes, vec![Mode::Text]);
        let math = math_def(&owner);
        assert_eq!(math.modes, vec![Mode::Math]);
        for def in [text, math] {
            let SyntaxDef::Macro { args } = &def.syntax else { panic!("a macro") };
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].name.as_deref(), Some(FORMULA_ARG));
            assert_eq!(args[0].mode, None, "the mode comes from the group or the formula");
        }
    }

    #[test]
    fn both_declarations_carry_the_one_element_type() {
        assert_eq!(element(Mode::Text).name, ENSUREMATH);
        assert_eq!(element(Mode::Math).name, ENSUREMATH);
    }
}
