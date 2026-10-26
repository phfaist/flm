//! The `literals` feature: the commands that stand for one character of text,
//! and `\@`.
//!
//! The guide to the language describes these commands for authors ([writing a
//! character that means something
//! else](crate::guide::language_syntax#writing-a-character-that-means-something-else)).
//!
//! # What an author writes
//!
//! A **literal** is a command, or a special character sequence, whose whole
//! meaning is one character of ordinary text. A special character sequence is a
//! sequence of characters recognized as a command without an escape character
//! before it ([`CallableKind`](crate::lang::mint::CallableKind)); `~` is the only
//! literal written that way. The feature declares nine literals:
//!
//! | written | character |
//! |---|---|
//! | `\%` | `%` |
//! | `\#` | `#` |
//! | `\&` | `&` |
//! | `\$` | `$` |
//! | `\ ` (a backslash and a space) | a space |
//! | `\{` | `{` |
//! | `\}` | `}` |
//! | `\textbackslash` | `\` |
//! | `~` | a non-breaking space, U+00A0 |
//!
//! All nine are declared so that one spelling of each character works in every
//! place of a document, and an author need not know where the character itself has
//! a meaning: `&` separates the cells of a table of cells, `#` marks a placeholder
//! in a user definition's template, `$` opens mathematics when the parsing option
//! [`FlmParsing::dollar_math`](crate::lang::FlmParsing::dollar_math) is on, and a
//! lone `%` is refused as ambiguous unless it starts a comment
//! ([`FlmParsing::percent_forbidden`](crate::lang::FlmParsing::percent_forbidden)).
//!
//! `\ ` produces a space, and `~` the non-breaking space U+00A0, which a backend
//! that lays text out keeps from breaking. The two are whitespace to the
//! layout of paragraphs (their element values carry the [`Whitespace`]
//! aspect): to the block-structure pass
//! ([`blocks`](crate::lang::rewrites::blocks)), and to the lowering helper
//! that trims the paragraphs and lines a rule writes over content it cuts out
//! itself
//! ([`stage_trimmed`](crate::stdcore::backends::lowering::stage_trimmed)).
//! They are removed where whitespace is removed: beside an element that
//! absorbs the whitespace next to it (`\cite` removes the whitespace written
//! before it), and at the start and at the end of a paragraph and of a line of
//! a quotation; and they start no paragraph. Everywhere else they are kept as
//! written. The pass collapses every run of whitespace in the source to one
//! space before any lowering rule runs, but it never merges a literal into the
//! text beside it: `a~~b` keeps its two non-breaking spaces, and `a\ \ b` its
//! two spaces. The lowering rule produces the character of each literal kept.
//!
//! `\@` adjusts the space after a period. Written after a period (`cf.\@ the
//! appendix`), it says that the period ends an abbreviation, not a sentence;
//! written before a period that follows a capital letter (`NASA\@.`), it says
//! that the period ends a sentence. It produces no character.
//!
//! # What the feature registers
//!
//! The `literals` feature ([`Literals`], a
//! [`FlmFeature`]) registers:
//!
//! - the element type [`LITERAL`] (`flm.literal`), with one element value per
//!   literal. The nine literals form one family, declared as one element type with
//!   one element value per command ([elements and
//!   aspects](crate::guide::concepts_overview#elements-and-aspects)): each
//!   element value carries a [`LiteralElement`], whose [`Literal`] says which
//!   character the literal stands for, and the element values of `\ ` and `~`
//!   carry the [`Whitespace`] aspect too;
//! - the element [`SENTENCE_SPACING`] (`flm.sentence-spacing`) for `\@`, with no
//!   data of its own;
//! - the default lowering rules [`RULE`] and [`SENTENCE_SPACING_RULE`], and the
//!   recomposition rules of [`latex`] for the `latex-recomposed` backend.
//!
//! # What a literal becomes on each backend
//!
//! - **Default rules** (used by the `text` and `html` backends): [`RULE`]
//!   replaces a literal by a text node that holds its one character
//!   ([`Literal::text`]); [`SENTENCE_SPACING_RULE`] replaces `\@` by nothing,
//!   because plain text and HTML have no way to write the hint.
//! - **`latex-recomposed` backend**: the rules of [`latex`] write every literal
//!   and `\@` back as the author wrote it.
//!
//! # Map of the module
//!
//! - [`Literals`] — the feature; [`feature`] returns it for a package entry.
//! - [`LITERAL`], [`Literal`], [`LiteralElement`] — the element type of the
//!   literals and the data of each element value.
//! - [`SENTENCE_SPACING`] — the element of `\@`.
//! - [`RULE`], [`LiteralRule`], [`SENTENCE_SPACING_RULE`],
//!   [`SentenceSpacingRule`] — the default lowering rules and their bodies.
//! - [`latex`] — the recomposition rules for the `latex-recomposed` backend.
//! - [`textstyles`](crate::stdcore::features::textstyles) — the sibling feature
//!   that declares the inline formatting commands.

// The spelling written for a value is what tells the nine apart in a dump,
// since the element type is one; see decision rationale at
// [§flm-dd-dr:std-element-families] and [§flm-dd-dr:std-literals].

pub mod latex;

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::lang::Flm;
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole, IsWhitespace, Whitespace};
use crate::model::element::{ElementSpec, element_value};

/// The feature's registered name, `literals`.
pub const FEATURE: &str = "literals";

/// The name of the element type of the nine literals, `flm.literal`. The data
/// of each element value is a [`LiteralElement`].
pub const LITERAL: &str = "flm.literal";

/// The name of the default lowering rule of [`LITERAL`] on every backend,
/// `flm.literal.lower.default`, whose body is [`LiteralRule`].
pub const RULE: &str = "flm.literal.lower.default";

/// The name of the element of `\@`, `flm.sentence-spacing`. The element
/// carries no data of its own.
pub const SENTENCE_SPACING: &str = "flm.sentence-spacing";

/// The name of the default lowering rule of [`SENTENCE_SPACING`] on every
/// backend, `flm.sentence-spacing.lower.default`, whose body is
/// [`SentenceSpacingRule`].
pub const SENTENCE_SPACING_RULE: &str = "flm.sentence-spacing.lower.default";

/// One of the nine literals: the command or special character sequence, and
/// the character it stands for.
///
/// [`Literal::text`] gives the character, [`Literal::source`] how the literal
/// is written, and [`Literal::command`] or [`Literal::trigger`] the name it is
/// declared under. The [module documentation](self) has the table of all
/// nine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Literal {
    /// `\%` — a percent sign.
    Percent,
    /// `\#` — a number sign.
    NumberSign,
    /// `\&` — an ampersand.
    Ampersand,
    /// `\$` — a dollar sign.
    Dollar,
    /// `\ ` (a backslash and a space) — a space that is never collapsed with
    /// the whitespace around it.
    Space,
    /// `\{` — an opening brace.
    OpenBrace,
    /// `\}` — a closing brace.
    CloseBrace,
    /// `\textbackslash` — a backslash.
    Backslash,
    /// `~` — a non-breaking space, U+00A0.
    NonBreakingSpace,
}

impl Literal {
    /// The nine literals, in the order the feature declares them.
    pub const ALL: [Literal; 9] = [
        Literal::Percent,
        Literal::NumberSign,
        Literal::Ampersand,
        Literal::Dollar,
        Literal::Space,
        Literal::OpenBrace,
        Literal::CloseBrace,
        Literal::Backslash,
        Literal::NonBreakingSpace,
    ];

    /// The character this literal stands for, as a string of one character:
    /// what [`LiteralRule`] writes into the text node it stages.
    pub fn text(self) -> &'static str {
        match self {
            Literal::Percent => "%",
            Literal::NumberSign => "#",
            Literal::Ampersand => "&",
            Literal::Dollar => "$",
            Literal::Space => " ",
            Literal::OpenBrace => "{",
            Literal::CloseBrace => "}",
            Literal::Backslash => "\\",
            Literal::NonBreakingSpace => "\u{a0}",
        }
    }

    /// How this literal is written in a document, escape character included
    /// (`"\\%"`, `"\\textbackslash"`, `"~"`): what the recomposition rule
    /// of [`latex`] writes.
    pub fn source(self) -> &'static str {
        match self {
            Literal::Percent => "\\%",
            Literal::NumberSign => "\\#",
            Literal::Ampersand => "\\&",
            Literal::Dollar => "\\$",
            Literal::Space => "\\ ",
            Literal::OpenBrace => "\\{",
            Literal::CloseBrace => "\\}",
            Literal::Backslash => "\\textbackslash",
            Literal::NonBreakingSpace => "~",
        }
    }

    /// The name of the macro this literal is declared as, without the escape
    /// character (`"%"`, `"textbackslash"`, `" "`), or `None` for
    /// [`Literal::NonBreakingSpace`], which is a special character sequence
    /// and has no macro name ([`Literal::trigger`] gives its characters).
    pub fn command(self) -> Option<&'static str> {
        match self {
            Literal::Percent => Some("%"),
            Literal::NumberSign => Some("#"),
            Literal::Ampersand => Some("&"),
            Literal::Dollar => Some("$"),
            Literal::Space => Some(" "),
            Literal::OpenBrace => Some("{"),
            Literal::CloseBrace => Some("}"),
            Literal::Backslash => Some("textbackslash"),
            Literal::NonBreakingSpace => None,
        }
    }

    /// The characters of the special character sequence this literal is
    /// declared as (`"~"`), or `None` for the eight literals that are
    /// macros.
    pub fn trigger(self) -> Option<&'static str> {
        match self {
            Literal::NonBreakingSpace => Some(self.source()),
            _ => None,
        }
    }

    /// The syntax this literal is declared with: a macro that takes no
    /// argument, or a special character sequence with its characters.
    fn syntax(self) -> SyntaxDef {
        match self.trigger() {
            Some(trigger) => {
                SyntaxDef::Specials { trigger: String::from(trigger), args: Vec::new() }
            }
            None => SyntaxDef::Macro { args: Vec::new() },
        }
    }
}

/// The data of one element value of `flm.literal`: the literal, and so the
/// character it stands for.
///
/// A lowering rule or a stage reads it from a node with [`element_value`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralElement {
    /// The literal, and so the character it produces.
    pub literal: Literal,
}

/// The element value of one literal: a `flm.literal` element value carrying
/// `literal`, inline (the default block role), and whitespace ([`Whitespace`])
/// for the two space literals, `\ ` and `~`.
fn element(literal: Literal) -> Arc<ElementSpec> {
    let element = ElementSpec::new(LITERAL)
        .docs(format!("The character `{}`, written `{}`.", literal.text(), literal.source()))
        .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
        .with_value(Arc::new(LiteralElement { literal }));
    match literal {
        Literal::Space | Literal::NonBreakingSpace => {
            Arc::new(element.with::<dyn Whitespace>(Arc::new(IsWhitespace)))
        }
        _ => Arc::new(element),
    }
}

/// The element value of `\@`: a `flm.sentence-spacing` element, inline (the
/// default block role), with no data of its own.
fn sentence_spacing_element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(SENTENCE_SPACING)
            .docs(
                "A hint about the space after a period: whether that period ends a sentence. \
                 It produces no character.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

/// The body of [`RULE`]: it replaces the literal by a text node that holds the
/// character the literal stands for ([`Literal::text`]).
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`LiteralElement`] or when the text node could not be staged.
#[derive(Debug, Default)]
pub struct LiteralRule;

impl Lower for LiteralRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<LiteralElement>(&node)
            .ok_or_else(|| LowerError::structure("a literal node without its value"))?;
        let text = lw.text(cx, value.literal.text())?;
        Ok(Lowering::Emit(vec![text]))
    }
}

/// The body of [`SENTENCE_SPACING_RULE`]: it replaces `\@` by nothing. The
/// hint has no form in plain text or HTML; the `latex-recomposed` backend
/// writes it back through the rule of [`latex`]. See the [module
/// documentation](self) for what `\@` means.
#[derive(Debug, Default)]
pub struct SentenceSpacingRule;

impl Lower for SentenceSpacingRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// The `literals` feature: it registers the nine literals and `\@`, their two
/// default lowering rules and the recomposition rules of [`latex`]. See the
/// module documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Literals;

impl FlmFeature for Literals {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Literal characters").docs(
            "The commands that stand for one character of text (`\\%`, `\\#`, `\\&`, `\\$`, \
             `\\ `, `\\{`, `\\}`, `\\textbackslash`) and the special character sequence `~` \
             for a non-breaking space: nine values of the element type `flm.literal`. Also \
             `\\@`, a hint about whether a period ends a sentence.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        for literal in Literal::ALL {
            b.element(
                ElementDef::new(
                    owner.clone(),
                    literal.command(),
                    literal.syntax(),
                    element(literal),
                )
                .docs(format!(
                    "`{}`: the character `{}`.",
                    literal.source(),
                    literal.text()
                )),
            )?;
        }
        b.element(
            ElementDef::new(
                owner.clone(),
                ["@"],
                SyntaxDef::Macro { args: Vec::new() },
                sentence_spacing_element(),
            )
            .docs(
                "`\\@`: a hint about whether a period ends a sentence. Only the recomposed \
                 LaTeX writes it; every other output leaves it out.",
            ),
        )?;
        b.lowering_rule(
            LoweringRule::new(
                RULE,
                LITERAL,
                LoweringDecl::new(),
                Arc::new(LiteralRule),
                DefaultMark::Everywhere,
            )
            .docs("a literal lowers to a text node holding the character it stands for"),
        )?;
        b.lowering_rule(
            LoweringRule::new(
                SENTENCE_SPACING_RULE,
                SENTENCE_SPACING,
                LoweringDecl::new(),
                Arc::new(SentenceSpacingRule),
                DefaultMark::Everywhere,
            )
            .docs("`\\@` lowers to nothing"),
        )?;
        latex::register(b)?;
        Ok(())
    }
}

/// The [`Literals`] feature as a shared value, ready to be listed in a package
/// entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Literals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spelling_names_its_source_and_its_character() {
        assert_eq!(Literal::Percent.source(), "\\%");
        assert_eq!(Literal::Percent.command(), Some("%"));
        assert_eq!(Literal::Backslash.command(), Some("textbackslash"));
        assert_eq!(Literal::Space.command(), Some(" "));
        assert_eq!(Literal::NonBreakingSpace.command(), None);
        assert_eq!(Literal::NonBreakingSpace.trigger(), Some("~"));
        assert_eq!(Literal::NonBreakingSpace.text(), "\u{a0}");
        assert_eq!(Literal::ALL.len(), 9);
        for literal in Literal::ALL {
            assert_eq!(literal.text().chars().count(), 1);
        }
    }

    #[test]
    fn an_element_value_carries_its_spelling_and_the_inline_role() {
        let element = element(Literal::OpenBrace);
        assert_eq!(element.name, LITERAL);
        assert_eq!(
            element.value_as::<LiteralElement>().map(|v| v.literal),
            Some(Literal::OpenBrace)
        );
        assert!(element.has_aspect::<dyn BlockRoleAspect>());
    }

    #[test]
    fn the_sentence_spacing_element_carries_no_value() {
        let element = sentence_spacing_element();
        assert_eq!(element.name, SENTENCE_SPACING);
        assert!(element.value_as::<LiteralElement>().is_none());
    }
}
