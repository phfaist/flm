//! The element declarations of the `quote` feature: one environment per
//! configured quote environment; the four quotation-section commands, in one
//! scoped package per environment that accepts any; and the two field macros
//! of a lines section, `\\` and `\indent` (macros that write nothing and that
//! the lowering rules read in place;
//! [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)),
//! in the scoped package [`LINES_PACKAGE`](super::LINES_PACKAGE). The module also holds the test of
//! section position ([`section_position`]), which the block role of a
//! quotation-section command and the lowering rules of a quotation share.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::lang::{Flm, FlmGroupType};
use crate::lang::mint::{
    ArgDef, ArgLayout, BodyDef, BodyLayout, ElementDef, ScopedPackageDef, SyntaxDef, field_element,
};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::{FeatureName, Name};
use crate::stdcore::backends::lowering::rules::owning_callable;

use super::config::{QuoteEnvironment, QuoteSection};
use super::{
    CONTENT_ARG, INDENT_COMMAND, LINEBREAK_COMMAND, LINES_PACKAGE, QUOTE, SECTION,
    sections_package,
};

/// The data that each element value of [`QUOTE`] carries: which quote
/// environment the value is, and what the configuration declares about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteElement {
    /// The environment's name, which is also the kind token of the block a
    /// quotation of this environment is staged as.
    pub name: String,
    /// What the configuration declares about the environment.
    pub environment: QuoteEnvironment,
}

/// The data that each element value of [`SECTION`] carries: which
/// quotation-section command the value is.
///
/// It also gives the command's block role ([`BlockRoleAspect`]), decided per
/// occurrence. A quotation-section command in *section position*, written
/// directly in the body of a quote environment that accepts it, is a block
/// where that body is laid out as paragraphs and blocks, so that it is a
/// block of its own between the others. Anywhere else it is inline content:
/// in a body laid out as inline content, whose quotation sections and bare
/// content form one run, and wherever it opens no quotation section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionElement {
    /// The kind of quotation section the command writes.
    pub section: QuoteSection,
}

impl BlockRoleAspect for SectionElement {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        match section_position(node) {
            Some(position) if position.quote.environment.holds_blocks() => BlockRole::Block,
            _ => BlockRole::DEFAULT,
        }
    }
}

/// A quotation-section command in section position ([`section_position`]):
/// the kind of quotation section it opens, and the quotation in whose body it
/// is written.
#[derive(Debug, Clone, Copy)]
pub(super) struct SectionPosition<'t> {
    /// The kind of quotation section the command opens.
    pub(super) section: QuoteSection,
    /// The quotation in whose body the command is written.
    pub(super) quote: &'t QuoteElement,
}

/// Returns the section position of `node`, or `None` when `node` is not in
/// **section position**: a quotation-section command written as a direct
/// child of the body of a quote environment that accepts the command.
///
/// This predicate decides both the command's block role ([`SectionElement`])
/// and which children of a body the lowering rules of a quotation stage as
/// quotation sections, so that the two always agree. It reads the tree each
/// time; nothing is stored.
pub(super) fn section_position<'t>(node: &NodeRef<'t, Flm>) -> Option<SectionPosition<'t>> {
    let section = element_value::<SectionElement>(node)?.section;
    let quote = quote_of_body(node)?;
    quote.environment.enables(section).then_some(SectionPosition { section, quote })
}

/// Returns the quotation in whose body `node` is written as a direct child,
/// if any.
fn quote_of_body<'t>(node: &NodeRef<'t, Flm>) -> Option<&'t QuoteElement> {
    // A quote environment takes no argument: whatever it owns is its body.
    owning_callable(node).and_then(|owner| element_value::<QuoteElement>(&owner))
}

/// Returns whether `node` is written directly in a quote environment's body:
/// as a direct child of the body, or of a paragraph that the block-structure
/// pass built directly in the body.
pub(super) fn is_in_quote_body(node: &NodeRef<'_, Flm>) -> bool {
    let place = match node.parent() {
        Some(parent) if parent.group_type() == Some(FlmGroupType::Paragraph) => parent,
        _ => *node,
    };
    quote_of_body(&place).is_some()
}

/// The element value of the quote environment `name`: a block, carrying the
/// environment.
fn quote_element(name: &str, environment: &QuoteEnvironment) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(QUOTE)
            .docs(format!("`{name}`: a quote environment, a quotation set apart from the running text."))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with_value(Arc::new(QuoteElement {
                name: name.to_string(),
                environment: environment.clone(),
            })),
    )
}

/// Returns the declaration of the quote environment `name`, owned by
/// `owner`: no argument, and a body laid out as paragraphs and blocks or as
/// inline content according to the kind its bare content is read as
/// ([`QuoteEnvironment::holds_blocks`]). The body refers to the scoped package
/// of its quotation-section commands where it accepts at least one, and to
/// the lines package where its bare content is read as lines. Text mode
/// only.
pub(super) fn environment_def(
    owner: &FeatureName,
    name: &str,
    environment: &QuoteEnvironment,
) -> ElementDef {
    let layout = if environment.holds_blocks() { BodyLayout::Blocks } else { BodyLayout::Inline };
    let mut body = BodyDef::new().layout(layout);
    if !environment.sections.is_empty() {
        body = body.scoped(sections_package(name));
    }
    if environment.bare == Some(QuoteSection::Lines) {
        body = body.scoped(LINES_PACKAGE);
    }
    let commands: Vec<String> =
        environment.sections.iter().map(|section| format!("`\\{}`", section.as_str())).collect();
    let docs = match (commands.is_empty(), environment.bare) {
        (false, _) => format!(
            "`\\begin{{{name}}}…\\end{{{name}}}`: a quotation whose quotation sections are written with {}.",
            commands.join(", ")
        ),
        (true, Some(kind)) => format!(
            "`\\begin{{{name}}}…\\end{{{name}}}`: a quotation whose content is one {} section.",
            kind.as_str()
        ),
        (true, None) => format!("`\\begin{{{name}}}…\\end{{{name}}}`: a quotation."),
    };
    ElementDef::new(
        owner.clone(),
        [name],
        SyntaxDef::Environment { args: Vec::new(), body },
        quote_element(name, environment),
    )
    .docs(docs)
}

/// Returns the element value of the quotation-section command of the kind
/// `section`, whose block role is decided per occurrence
/// ([`SectionElement`]).
fn section_element(section: QuoteSection) -> Arc<ElementSpec> {
    let value = Arc::new(SectionElement { section });
    Arc::new(
        ElementSpec::new(SECTION)
            .docs(format!("`\\{}{{…}}`: a {} section of a quotation.", section.as_str(), section.as_str()))
            .with::<dyn BlockRoleAspect>(Arc::clone(&value) as Arc<dyn BlockRoleAspect>)
            .with_value(value),
    )
}

/// Returns the declaration of the quotation-section command of the kind
/// `section`, owned by `owner`: one mandatory argument, [`CONTENT_ARG`],
/// laid out as inline content for a passage, lines and an attribution, and
/// as inline content or as paragraphs and blocks, according to its content,
/// for a block section; the argument of a lines section refers to the lines
/// package. Text mode only, so that `\text{…}` inside mathematics is
/// mathematics' own text command.
fn section_def(owner: &FeatureName, section: QuoteSection, element: &Arc<ElementSpec>) -> ElementDef {
    let mut content = ArgDef::mandatory(CONTENT_ARG);
    let docs = match section {
        QuoteSection::Text => "The quoted passage, one paragraph.",
        QuoteSection::Lines => {
            content = content.scoped(LINES_PACKAGE);
            "The lines, each ended by `\\\\`; `\\indent` at the start of a line indents it by a \
             tab."
        }
        QuoteSection::Attributed => "The attribution: an author, a source.",
        QuoteSection::Block => {
            content = content.layout(ArgLayout::InlineOrBlocks);
            "Paragraphs and blocks; content written on one line is one paragraph."
        }
    };
    let name = section.as_str();
    ElementDef::new(
        owner.clone(),
        [name],
        SyntaxDef::Macro { args: vec![content.docs(docs)] },
        Arc::clone(element),
    )
    .docs(format!("`\\{name}{{…}}`: a {name} section of the quotation around it."))
}

/// Returns the scoped packages of the quotation-section commands, declared by
/// `owner`: one per environment of `environments` that accepts at least one
/// command, whose name [`sections_package`] returns, holding exactly the commands
/// that environment accepts.
///
/// A quotation-section command is one element value of [`SECTION`], however
/// many packages hold it: two environments that accept `\text` declare the
/// same element value under the same name in two packages. The two
/// declarations never clash, because a scoped package is available only
/// inside the bodies and arguments whose declarations refer to it.
pub(super) fn sections_packages<'c>(
    owner: &FeatureName,
    environments: impl IntoIterator<Item = (&'c String, &'c QuoteEnvironment)>,
) -> Vec<ScopedPackageDef> {
    let elements: Vec<(QuoteSection, Arc<ElementSpec>)> =
        QuoteSection::ALL.into_iter().map(|section| (section, section_element(section))).collect();
    environments
        .into_iter()
        .filter(|(_, environment)| !environment.sections.is_empty())
        .map(|(name, environment)| {
            let defs = environment
                .sections
                .iter()
                .filter_map(|section| elements.iter().find(|(kind, _)| kind == section))
                .map(|(section, element)| section_def(owner, *section, element))
                .collect();
            ScopedPackageDef {
                owner: owner.clone(),
                name: Name::from(sections_package(name)),
                elements: defs,
                docs: format!(
                    "The quotation-section commands available inside the body of the quote \
                     environment `{name}`."
                ),
            }
        })
        .collect()
}

/// Returns the scoped package [`LINES_PACKAGE`], declared by `owner`: the
/// field macros `\\`, which ends a line, and `\indent`, which indents the
/// line it starts by one tab. Text mode only. The lowering rules of a lines
/// section read both in place among the section's content; neither writes
/// anything of its own.
pub(super) fn lines_package(owner: &FeatureName) -> ScopedPackageDef {
    let linebreak = ElementDef::new(
        owner.clone(),
        [LINEBREAK_COMMAND],
        SyntaxDef::Macro { args: Vec::new() },
        field_element(
            LINEBREAK_COMMAND,
            "A `\\\\` inside a lines section of a quotation: the end of a line.",
        ),
    )
    .docs("`\\\\`: ends a line of the lines section.");
    let indent = ElementDef::new(
        owner.clone(),
        [INDENT_COMMAND],
        SyntaxDef::Macro { args: Vec::new() },
        field_element(
            INDENT_COMMAND,
            "An `\\indent` at the start of a line of a lines section of a quotation: one tab of \
             indentation for the line.",
        ),
    )
    .docs("`\\indent`: indents the line it starts by one tab; written several times, by as many.");
    ScopedPackageDef {
        owner: owner.clone(),
        name: Name::from(LINES_PACKAGE),
        elements: vec![linebreak, indent],
        docs: String::from(
            "The commands available inside a lines section of a quotation: `\\\\`, which ends a \
             line, and `\\indent`, which indents the line it starts.",
        ),
    }
}
