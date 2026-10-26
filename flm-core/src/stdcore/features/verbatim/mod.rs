//! The `verbatim` feature: content shown exactly as the author wrote it, through
//! three macros and two environments.
//!
//! The guide to the language describes verbatim content for authors
//! ([verbatim content](crate::guide::language_syntax#verbatim-content)).
//!
//! # What an author writes
//!
//! Verbatim content is a [verbatim
//! region](crate::guide::concepts_overview#verbatim-regions): a run of raw text in
//! which no markup character means anything. A backslash, a brace and a percent
//! sign are ordinary characters, and the region ends only at its own
//! terminator. The feature declares five commands:
//!
//! | written | element | verbatim kind | display |
//! |---|---|---|---|
//! | `\verbtext{…}` | `flm.verbatim.macro` | `text` | inline |
//! | `\verbcode[lang]{…}` | `flm.verbatim.macro` | `code` | inline |
//! | `\verba[lang]{…}` | `flm.verbatim.macro` | `a` | inline |
//! | `\begin{verbatimtext}…\end{verbatimtext}` | `flm.verbatim.environment` | `text` | inline |
//! | `\begin{verbatimcode}[lang]…\end{verbatimcode}` | `flm.verbatim.environment` | `code` | block |
//!
//! A **verbatim kind** ([`VerbatimKind`]) says what sort of content a verbatim
//! region holds: characters with no formatting of their own (`text`), computer
//! code (`code`), or an identifier such as a keyword or a name (`a`, after the
//! macro `\verba`). `{verbatimcode}` is the only one of the five commands that
//! is a block of its own; the others are inline content of the paragraph they
//! are written in.
//!
//! A macro's content is delimited by the first character after the command (and
//! after its optional language argument): the region closes at the partner of
//! that character for `{`, `[`, `<` and `(`, counting nested pairs, so that it
//! ends at the partner that balances the opening one; for any other character it
//! closes at the same character again, and the content may then hold unbalanced
//! braces. `\verbcode+a{b+` and `\verbcode{a{b}c}` both read the characters
//! between the delimiters. The optional argument `[lang]` gives the
//! programming language the content is written in; no standard feature
//! interprets it, and a backend that highlights syntax may use it.
//!
//! # What the feature registers
//!
//! The `verbatim` feature ([`Verbatim`], a
//! [`FlmFeature`]) registers:
//!
//! - the element types [`MACRO`] (`flm.verbatim.macro`) for the three macros and
//!   [`ENVIRONMENT`] (`flm.verbatim.environment`) for the two environments, with
//!   one element value per command, each carrying a [`VerbatimCallable`] (the
//!   command's name, its verbatim kind, whether it takes a language argument,
//!   whether it is a block). The commands are declared as a family, one element
//!   type with one element value per command ([elements and
//!   aspects](crate::guide::concepts_overview#elements-and-aspects)); the two
//!   forms get two element types because a macro holds its content in an
//!   argument and an environment in its body, so each rule reads one shape;
//! - the default lowering rules [`MACRO_RULE`] and [`ENVIRONMENT_RULE`], and the
//!   recomposition rules of [`latex`] for the `latex-recomposed` backend.
//!
//! The commands are declared with the language layer's declarations for raw text
//! ([`ArgStdKind::Verbatim`] for a macro's content, [`BodyDef::verbatim`] for an
//! environment's body), so the parse wraps every verbatim run in a group node of
//! the verbatim class, as [`crate::lang::verbatim`] describes; the feature writes
//! no parser of its own.
//!
//! # What a verbatim command becomes on each backend
//!
//! - **Default rules** ([`MACRO_RULE`], [`ENVIRONMENT_RULE`], used by the `text`
//!   and `html` backends): each stages (builds into the lowered tree; see
//!   [`Lowerer`]) a
//!   [`flm.pres.verbatim`](crate::stdcore::backends::pres::VERBATIM) node whose
//!   payload carries the verbatim kind, the display (inline or block) and the
//!   language, over the region's characters copied unchanged. Nothing inside a
//!   verbatim region is lowered through the rules.
//! - **`text` backend**: it prints the characters byte for byte: inline content
//!   as one unit that is never broken across lines (or as a block when it
//!   holds a newline or a tab), a block on lines of its own; it prints neither
//!   the verbatim kind nor the language.
//! - **`html` backend**: it writes the node as the [HTML
//!   contract](crate::guide::rendering#the-html-contract) states for
//!   `flm.pres.verbatim` ([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)):
//!   a `<code>` element inline and a `<pre><code>` pair for a block, with the
//!   verbatim kind, the display and the language as attributes.
//! - **`latex-recomposed` backend**: the rules of [`latex`] write inline content
//!   through the `fvextra` package's `\EscVerb` command and a block through the
//!   environment `flmVerbatimCode`. Each verbatim kind is set through its
//!   **formatting hook**: a [LaTeX hook](crate::stdcore::backends::latex::preamble)
//!   (a macro that the style file defines with a plain default and a user
//!   redefines) named `\flmFmtVRBverbtext`, `\flmFmtVRBverbcode` or
//!   `\flmFmtVRBverba` after the macro of that kind.
//!
//! # Content that is not characters
//!
//! What an author writes between a verbatim command's delimiters is characters.
//! A user definition's template may place one of its arguments inside verbatim
//! content, though, and its expansion then puts that argument's content there as
//! it is: with a user definition `\code` whose template is `\verbcode{#1}`,
//! the verbatim content of `\code{…}` is the content of its argument, which
//! may hold any node a document can write. Every rule of this feature
//! therefore reads the content as characters through
//! [`read_verbatim_content`]: a text node gives its characters, a comment gives
//! nothing, and a brace group gives its content without the braces, so
//! `\code{a{b}c}` shows `abc`. A command, an environment, a special character
//! sequence or mathematics has no characters to give: the Error
//! [`ContentNotCharacters`] is reported at the verbatim command or environment,
//! and the region shows its whole content written as its source, so
//! `\code{\emph{x}}` shows `\emph{x}`. Every backend does the same, the
//! recomposed LaTeX included.
//!
//! [`read_verbatim_content`]: crate::stdcore::backends::lowering::read_verbatim_content
//! [`ContentNotCharacters`]: crate::stdcore::backends::lowering::ContentNotCharacters
//!
//! # Map of the module
//!
//! - [`Verbatim`] — the feature; [`feature`] returns it for a package entry.
//! - [`MACRO`], [`ENVIRONMENT`], [`VerbatimCallable`] — the element types and the
//!   data of each element value; [`MACROS`] and [`ENVIRONMENTS`] list the five
//!   commands.
//! - [`MACRO_RULE`], [`MacroRule`], [`ENVIRONMENT_RULE`], [`EnvironmentRule`] —
//!   the default lowering rules and their bodies.
//! - [`CONTENT_ARG`], [`LANG_ARG`] — the names of the arguments.
//! - [`latex`] — the recomposition rules for the `latex-recomposed` backend.

// The element family shape and the feature's own vocabulary: see design
// rationale at [§flm-dd-dr:std-verbatim] and [§flm-dd-dr:std-element-families].
// The language argument is an ordinary optional argument rather than the
// characters-only group Python's parser requires immediately after the
// command: the declarative argument vocabulary has no optional
// characters-only shape, and adding one to the language layer for one
// argument is not warranted; cf. [§flm-dd-dr:lang-restricted-shapes].

pub mod latex;

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::model::names::{ElementName, FeatureName, Name};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{ArgDef, ArgStdKind, BodyDef, ElementDef, SyntaxDef};
use crate::lang::text::is_space;
use crate::lang::{Flm, FlmGroupType};
use crate::stdcore::backends::lowering::{copy_subtree_verbatim, stage_verbatim_content};
use crate::stdcore::backends::pres::{
    self, VerbatimDisplay, VerbatimKind, VerbatimPayload, plain_text,
};

/// The feature's registered name, `verbatim`.
pub const FEATURE: &str = "verbatim";

/// The name of the element type of the three verbatim macros,
/// `flm.verbatim.macro`. The data of each element value is a
/// [`VerbatimCallable`].
pub const MACRO: &str = "flm.verbatim.macro";

/// The name of the element type of the two verbatim environments,
/// `flm.verbatim.environment`. The data of each element value is a
/// [`VerbatimCallable`].
pub const ENVIRONMENT: &str = "flm.verbatim.environment";

/// The name of the default lowering rule of [`MACRO`] on every backend,
/// `flm.verbatim.macro.lower.default`, whose body is [`MacroRule`].
pub const MACRO_RULE: &str = "flm.verbatim.macro.lower.default";

/// The name of the default lowering rule of [`ENVIRONMENT`] on every backend,
/// `flm.verbatim.environment.lower.default`, whose body is
/// [`EnvironmentRule`].
pub const ENVIRONMENT_RULE: &str = "flm.verbatim.environment.lower.default";

/// The name of the argument that holds a verbatim macro's content,
/// `content`.
pub const CONTENT_ARG: &str = "content";

/// The name of the optional argument that gives the programming language of
/// the content, `lang`.
pub const LANG_ARG: &str = "lang";

/// The verbatim macros the feature declares, as `(name, verbatim kind, takes
/// a language argument)`.
pub const MACROS: [(&str, VerbatimKind, bool); 3] = [
    ("verbtext", VerbatimKind::Text, false),
    ("verbcode", VerbatimKind::Code, true),
    ("verba", VerbatimKind::Identifier, true),
];

/// The verbatim environments the feature declares, as `(name, verbatim kind,
/// takes a language argument, is a block of its own)`.
pub const ENVIRONMENTS: [(&str, VerbatimKind, bool, bool); 2] = [
    ("verbatimtext", VerbatimKind::Text, false, false),
    ("verbatimcode", VerbatimKind::Code, true, true),
];

/// The data of one element value of `flm.verbatim.macro` or
/// `flm.verbatim.environment`: which verbatim command was written and what it
/// means.
///
/// A lowering rule or a stage reads it from a node with [`element_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimCallable {
    /// The callable's name as written (`verbcode`, `verbatimcode`).
    pub name: Name,
    /// The verbatim kind of the content it introduces.
    pub kind: VerbatimKind,
    /// Whether it takes the optional `[lang]` argument.
    pub lang_arg: bool,
    /// Whether its content is a block of its own (`true` for
    /// `{verbatimcode}` only).
    pub block: bool,
}

/// The element value of one verbatim macro: inline content of the verbatim
/// kind `kind`.
fn macro_element(name: &str, kind: VerbatimKind, lang_arg: bool) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(MACRO)
            .docs(format!("`\\{name}`: inline verbatim content of kind `{kind}`."))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with_value(Arc::new(VerbatimCallable {
                name: Name::from(name),
                kind,
                lang_arg,
                block: false,
            })),
    )
}

/// The element value of one verbatim environment: content of the verbatim
/// kind `kind`, a block of its own when `block`.
fn environment_element(
    name: &str,
    kind: VerbatimKind,
    lang_arg: bool,
    block: bool,
) -> Arc<ElementSpec> {
    let role = if block { BlockRole::Block } else { BlockRole::DEFAULT };
    Arc::new(
        ElementSpec::new(ENVIRONMENT)
            .docs(format!(
                "`\\begin{{{name}}}…\\end{{{name}}}`: verbatim content of kind `{kind}`."
            ))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(role)))
            .with_value(Arc::new(VerbatimCallable {
                name: Name::from(name),
                kind,
                lang_arg,
                block,
            })),
    )
}

/// The arguments of a verbatim macro: the optional language argument when the
/// callable takes one, then the delimited verbatim content.
fn macro_args(lang_arg: bool) -> Vec<ArgDef> {
    let mut args = Vec::new();
    if lang_arg {
        args.push(language_argument());
    }
    args.push(
        ArgDef::named(CONTENT_ARG, ArgStdKind::Verbatim { delimiters: None }).docs(
            "The content, shown exactly as written. The first character after the command \
             opens the region, and its partner closes it (`}` for `{`, `]` for `[`, `>` for \
             `<`, `)` for `(`, nested pairs counted; the same character for any other).",
        ),
    );
    args
}

/// The declaration of the optional `[lang]` argument, shared by the commands
/// that take one.
fn language_argument() -> ArgDef {
    ArgDef::optional(LANG_ARG).docs(
        "The programming language the content is written in. A backend that highlights \
         syntax may use it; the text backend does not print it.",
    )
}

/// The language written on a verbatim node, trimmed of the whitespace around
/// it: `None` when the node's element declares no language argument, when the
/// argument is absent, or when its text is blank.
pub(super) fn language_of(node: &NodeRef<'_, Flm>) -> Option<String> {
    let content = node.argument_content_nodes_named(LANG_ARG).ok()??;
    let text = plain_text(content.iter()).trim_matches(is_space).to_string();
    (!text.is_empty()).then_some(text)
}

/// The verbatim kind and the display of a verbatim node, read from its
/// element value; a [`LowerError`] for a node that carries no
/// [`VerbatimCallable`].
fn kind_and_display(
    node: &NodeRef<'_, Flm>,
) -> Result<(VerbatimKind, VerbatimDisplay), LowerError> {
    let value = element_value::<VerbatimCallable>(node)
        .ok_or_else(|| LowerError::structure("a verbatim node without its value"))?;
    let display =
        if value.block { VerbatimDisplay::Block } else { VerbatimDisplay::Inline };
    Ok((value.kind, display))
}

/// The body of [`MACRO_RULE`]: it stages an inline
/// [`flm.pres.verbatim`](crate::stdcore::backends::pres::VERBATIM) node,
/// carrying the verbatim kind and the language, over the characters of the
/// macro's content argument copied unchanged (for content that is not
/// characters, see the module documentation, *Content that is not
/// characters*).
///
/// When the content argument is missing (a parse recovery left it absent, for
/// instance for a region that the end of the input cut short, and the parse
/// reported it), the node is empty and nothing more is reported.
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`VerbatimCallable`] or when the content could not be staged.
#[derive(Debug, Default)]
pub struct MacroRule;

impl Lower for MacroRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let (kind, display) = kind_and_display(&node)?;
        let lang = language_of(&node);
        let body = match node.argument_content_nodes_named(CONTENT_ARG) {
            Ok(Some(content)) => stage_verbatim_content(&node, content, cx, lw)?,
            _ => Vec::new(),
        };
        let payload = VerbatimPayload { kind, display, lang };
        let verbatim = lw.pres_with(cx, &ElementName::from(pres::VERBATIM), payload, body)?;
        Ok(Lowering::Emit(vec![verbatim]))
    }
}

/// The body of [`ENVIRONMENT_RULE`]: it stages a
/// [`flm.pres.verbatim`](crate::stdcore::backends::pres::VERBATIM) node,
/// carrying the verbatim kind, the display and the language, over the
/// characters of the environment's body copied unchanged: a block for
/// `{verbatimcode}`, inline for `{verbatimtext}` (for content that is not
/// characters, see the module documentation, *Content that is not
/// characters*).
///
/// The body of a verbatim environment holds the group node of the verbatim
/// class that the parse puts around every verbatim run. The rule copies that
/// group's children, not the group itself: the node the rule stages already
/// represents the region, and lowering the group would stage a second
/// `flm.pres.verbatim` node inside it.
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`VerbatimCallable`] or when the content could not be staged.
#[derive(Debug, Default)]
pub struct EnvironmentRule;

impl Lower for EnvironmentRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let (kind, display) = kind_and_display(&node)?;
        let lang = language_of(&node);
        let mut body = Vec::new();
        if let Some(content) = node.body() {
            for child in content.iter() {
                if child.group_type() == Some(FlmGroupType::Verbatim) {
                    body.extend(stage_verbatim_content(&node, child.children(), cx, lw)?);
                } else {
                    body.push(copy_subtree_verbatim(cx, child)?);
                }
            }
        }
        let payload = VerbatimPayload { kind, display, lang };
        let verbatim = lw.pres_with(cx, &ElementName::from(pres::VERBATIM), payload, body)?;
        Ok(Lowering::Emit(vec![verbatim]))
    }
}

/// The `verbatim` feature: it registers the three verbatim macros, the two
/// verbatim environments, their two default lowering rules and the
/// recomposition rules of [`latex`]. See the module documentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Verbatim;

impl FlmFeature for Verbatim {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Verbatim content").docs(
            "Content shown exactly as written: the macros `\\verbtext`, `\\verbcode` and \
             `\\verba` and the environments `verbatimtext` and `verbatimcode`, each lowered to \
             a `flm.pres.verbatim` node that carries its verbatim kind and its language.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        for (name, kind, lang_arg) in MACROS {
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [name],
                    SyntaxDef::Macro { args: macro_args(lang_arg) },
                    macro_element(name, kind, lang_arg),
                )
                .docs(format!(
                    "`\\{name}…`: inline verbatim content of kind `{kind}`, between a \
                     delimiter character and its partner."
                )),
            )?;
        }
        for (name, kind, lang_arg, block) in ENVIRONMENTS {
            let args = if lang_arg { vec![language_argument()] } else { Vec::new() };
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [name],
                    SyntaxDef::Environment { args, body: BodyDef::new().verbatim() },
                    environment_element(name, kind, lang_arg, block),
                )
                .docs(format!(
                    "`\\begin{{{name}}}…\\end{{{name}}}`: verbatim content of kind `{kind}`, \
                     {}.",
                    if block { "a block of its own" } else { "inline" }
                )),
            )?;
        }
        b.lowering_rule(
            LoweringRule::new(
                MACRO_RULE,
                MACRO,
                LoweringDecl::new().produces(pres::VERBATIM),
                Arc::new(MacroRule),
                DefaultMark::Everywhere,
            )
            .docs(
                "a verbatim macro lowers to an inline `flm.pres.verbatim` node carrying its \
                 verbatim kind and language, over the characters of its content argument",
            ),
        )?;
        b.lowering_rule(
            LoweringRule::new(
                ENVIRONMENT_RULE,
                ENVIRONMENT,
                LoweringDecl::new().produces(pres::VERBATIM),
                Arc::new(EnvironmentRule),
                DefaultMark::Everywhere,
            )
            .docs(
                "a verbatim environment lowers to a `flm.pres.verbatim` node carrying its \
                 verbatim kind, display and language, over the characters of its body",
            ),
        )?;
        latex::register(b)?;
        Ok(())
    }
}

/// The [`Verbatim`] feature as a shared value, ready to be listed in a package
/// entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Verbatim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_callables_are_pythons_five() {
        let macros: Vec<&str> = MACROS.iter().map(|(name, _, _)| *name).collect();
        assert_eq!(macros, ["verbtext", "verbcode", "verba"]);
        let environments: Vec<&str> = ENVIRONMENTS.iter().map(|(name, _, _, _)| *name).collect();
        assert_eq!(environments, ["verbatimtext", "verbatimcode"]);
    }

    #[test]
    fn only_the_code_environment_is_a_block() {
        let element = environment_element("verbatimcode", VerbatimKind::Code, true, true);
        assert_eq!(element.name, ENVIRONMENT);
        let value = element.value_as::<VerbatimCallable>().expect("valued");
        assert!(value.block);
        let text = environment_element("verbatimtext", VerbatimKind::Text, false, false);
        assert!(!text.value_as::<VerbatimCallable>().expect("valued").block);
    }

    #[test]
    fn a_macro_takes_its_language_argument_before_its_content() {
        let named: Vec<Option<String>> =
            macro_args(true).iter().map(|arg| arg.name.clone()).collect();
        assert_eq!(named, [Some(String::from(LANG_ARG)), Some(String::from(CONTENT_ARG))]);
        let plain: Vec<Option<String>> =
            macro_args(false).iter().map(|arg| arg.name.clone()).collect();
        assert_eq!(plain, [Some(String::from(CONTENT_ARG))]);
    }
}
