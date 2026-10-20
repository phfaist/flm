//! The text commands of math mode: `\text{…}` and the related commands, which
//! put document text inside a formula.
//!
//! A **text command** is a command of math mode whose one argument is read in
//! text mode: inside a formula, its argument is document content rather than
//! mathematical notation. An author writes a word of text inside a formula, such
//! as "for all *x* such that", a unit or a short remark, with `\text{…}`,
//! `\textbf{…}`, `\mbox{…}` or another of the twelve commands of [`COMMANDS`].
//! Everything else in a formula is carried to the output as the author wrote it
//! (see the [`math`](super) feature).
//!
//! # Where the commands are declared
//!
//! This module declares the twelve commands **for math mode only**
//! (`[Mode::Math]`). In text mode they are not this module's: `\textbf`,
//! `\textit` and `\texttt` are the
//! [`textstyles`](crate::stdcore::features::textstyles) feature's, and `\text`,
//! `\mbox` and the others are unknown commands, an Error in text mode.
//!
//! The `textstyles` feature declares `\textbf`, `\textit` and `\texttt` in text
//! mode only, the default of a declaration ([`ElementDef::modes`]). The two
//! features therefore declare these names in modes that do not overlap: in text
//! mode the name is the `textstyles` feature's, in math mode this feature's.
//! Two declarations whose modes do not overlap never clash, so neither feature
//! declares an override, and the order in which the two features are registered
//! does not matter.
//!
//! # What the argument holds
//!
//! The argument of a text command is read in text mode, with the parsing rules of
//! text, so it holds ordinary FLM content: formatting commands (`\emph{…}`),
//! references (`\ref{…}`), uses of user definitions, and mathematics again
//! (`\(…\)`). The two modes differ at the argument's edge: a command that no
//! feature declares is kept as source in the mathematics around the argument,
//! but is an Error inside it, because the argument is text.
//!
//! Three later steps follow from this, each in its own module:
//!
//! - the [block-structure pass](crate::lang::rewrites::blocks) lays the argument
//!   out as inline content (whitespace resolved, typographic substitution
//!   applied), although the formula around it is an [opaque
//!   region](crate::guide::concepts_overview#opaque-regions) that the pass leaves
//!   untouched;
//! - on the `html` backend, and inside a display-math environment on the `text`
//!   backend, the lowering puts the argument's content inside a
//!   [`flm.pres.math-text`](crate::stdcore::backends::pres::MATH_TEXT) node among
//!   the formula's source, the only part of the formula that is content (on the
//!   `text` backend, `\(…\)` and `\[…\]` are copied whole as source);
//! - the `latex-recomposed` backend writes the command back as the LaTeX macro of
//!   the same name, its argument recomposed as text ([`super::latex`]).
//!
//! # The element and its rules
//!
//! The twelve commands form one family, declared as one element type with one
//! element value per command ([elements and
//! aspects](crate::guide::concepts_overview#elements-and-aspects)): the element
//! type [`TEXT`] (`flm.math.text`), each element value carrying a
//! [`MathTextCommand`] with the command's name, which a backend writes back.
//!
//! - [`RULE`] (`flm.math.text.lower.default`) is the default rule on every
//!   backend: it stages (builds into the lowered tree; see
//!   [`Lowerer`]) a `flm.pres.math-text` node
//!   that carries the command's name, over the argument lowered through the
//!   rules. The rules of the [`math`](super) feature that write a formula on the
//!   `text` and `html` backends lower each text command they find through it.
//! - On the `latex-recomposed` backend,
//!   [`TEXT_LATEX_RULE`](super::latex::TEXT_LATEX_RULE) is the default instead.
//!
//! # Commands not declared here
//!
//! `\intertext` and `\shortintertext` also take text, but they set a paragraph
//! of text between two lines of a display-math environment, not content inside a
//! formula. FLM does not model them: the reader of equation lines
//! ([`super::lines::equation_lines`]) would leave such a command in the content
//! of the line that follows it. They are therefore unknown commands, kept as
//! source like any unknown command in mathematics.

// Why the commands are the math feature's and declared for math mode only, why
// the argument's mode is part of the declaration, and which commands were left
// out: see decision rationale at [§flm-dd-dr:std-math-text-commands].

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::latexlike::Mode;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::{ElementName, FeatureName, Name};
use crate::stdcore::backends::lowering::lower_arg_or_nothing;
use crate::stdcore::backends::pres::{self, MathTextPayload};

/// The name of the element type of the text commands of math mode,
/// `flm.math.text`. The data of each element value is a
/// [`MathTextCommand`].
pub const TEXT: &str = "flm.math.text";

/// The name of the default lowering rule of [`TEXT`] on every backend,
/// `flm.math.text.lower.default`, whose body is [`TextRule`].
pub const RULE: &str = "flm.math.text.lower.default";

/// The name of the only argument of every text command, `content`.
pub const CONTENT_ARG: &str = "content";

/// The text commands declared for math mode, in the order they are registered:
/// `text`, `textnormal`, `textup`, `textrm`, `textmd`, `textbf`, `textit`,
/// `textsl`, `textsc`, `textsf`, `texttt`, `mbox`.
///
/// The names are written without the leading backslash. Each command takes one
/// mandatory argument read in text mode. The library does not apply what a
/// command asks of the output (a bold weight for `\textbf`, nothing for
/// `\text`): the command's name is carried by the element value
/// ([`MathTextCommand`]) and by the payload of the presentation node
/// ([`MathTextPayload::command`]), and the output decides.
pub const COMMANDS: [&str; 12] = [
    "text",
    "textnormal",
    "textup",
    "textrm",
    "textmd",
    "textbf",
    "textit",
    "textsl",
    "textsc",
    "textsf",
    "texttt",
    "mbox",
];

/// The data of one element value of `flm.math.text`: which text command was
/// written.
///
/// A lowering rule or a stage reads it from a node with [`element_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathTextCommand {
    /// The command's name as written, without the leading backslash
    /// (`textbf`).
    pub command: Name,
}

/// The element value of one text command: a `flm.math.text` element value
/// carrying the command's name, inline (the default block role).
fn text_element(command: &str) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(TEXT)
            .docs(format!(
                "`\\{command}{{…}}` inside mathematics: its argument is document content, read \
                 in text mode."
            ))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with_value(Arc::new(MathTextCommand { command: Name::from(command) })),
    )
}

/// The declaration of one text command: a macro, declared for math mode only,
/// with one mandatory argument read in text mode.
fn text_def(owner: &FeatureName, command: &str) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [command],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(CONTENT_ARG, ArgStdKind::Mandatory)
                    .mode(Mode::Text)
                    .docs("The text, as document content."),
            ],
        },
        text_element(command),
    )
    .modes([Mode::Math])
    .docs(format!(
        "`\\{command}{{text}}`: text inside a formula. The argument is read in text mode, \
         so it holds document content, not mathematical notation."
    ))
}

/// The body of [`RULE`]: it stages a
/// [`flm.pres.math-text`](crate::stdcore::backends::pres::MATH_TEXT) node that
/// carries the command's name, over the command's argument lowered through the
/// rules.
///
/// When the argument is missing (a parse recovery left it absent and the parse
/// reported it), the node has no content and nothing more is reported. The
/// rule returns a [`LowerError`] when the node carries no [`MathTextCommand`]
/// or when the argument or the node could not be lowered or staged.
#[derive(Debug, Default)]
pub struct TextRule;

impl Lower for TextRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<MathTextCommand>(&node)
            .ok_or_else(|| LowerError::structure("a math text command without its element's data"))?;
        let body = lower_arg_or_nothing(cx, lw, &node, 0)?;
        let payload = MathTextPayload {
            command: Some(value.command.to_string()),
            anchor: None,
            at_tag: false,
        };
        let text = lw.pres_with(cx, &ElementName::from(pres::MATH_TEXT), payload, body)?;
        Ok(Lowering::Emit(vec![text]))
    }
}

/// Registers the text commands and their default lowering rule, as part of the
/// `math` feature.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let owner: FeatureName = b.feature().clone();
    for command in COMMANDS {
        b.element(text_def(&owner, command))?;
    }
    b.lowering_rule(
        LoweringRule::new(
            RULE,
            TEXT,
            LoweringDecl::new().produces(pres::MATH_TEXT),
            Arc::new(TextRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "a text command of math mode lowers to a `flm.pres.math-text` node that carries \
             its name, over its argument lowered through the rules",
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_one_value_of_the_one_element_type() {
        for command in COMMANDS {
            let element = text_element(command);
            assert_eq!(element.name, TEXT);
            let value = element.value_as::<MathTextCommand>().expect("valued");
            assert_eq!(value.command, command);
        }
    }

    #[test]
    fn a_text_command_is_a_math_mode_macro_whose_argument_is_text_mode() {
        let def = text_def(&FeatureName::from("math"), "text");
        assert_eq!(def.modes, vec![Mode::Math]);
        let SyntaxDef::Macro { args } = &def.syntax else { panic!("a macro") };
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name.as_deref(), Some(CONTENT_ARG));
        assert_eq!(args[0].mode, Some(Mode::Text));
    }
}
