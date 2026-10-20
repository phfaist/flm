//! The `href` feature: hyperlinks to an address outside the document, written
//! `\href{address}{text}`, `\url{address}` and `\email{address}`.
//!
//! # What an author writes, and what it produces
//!
//! An **address** is the target of a link outside the document, as the author wrote
//! it: `https://example.com/page`, `someone@example.com`. The link's **target** is the
//! address with what the command writes before it: `mailto:` for `\email`, nothing for
//! `\href` and `\url` ([`HrefCommand::prefix`]). The **display** of a link is what a
//! reader sees of it:
//!
//! | written | links to | shows |
//! |---|---|---|
//! | `\href{https://example.com}{Click here}` | `https://example.com` | `Click here` |
//! | `\url{https://example.com/}` | `https://example.com/` | `example.com` |
//! | `\email{someone@example.com}` | `mailto:someone@example.com` | `someone@example.com` |
//!
//! The three commands can be written in text mode only. The guide to the language
//! describes them for authors ([hyperlinks](crate::guide::language_syntax#hyperlinks)).
//!
//! On each output, a link becomes:
//!
//! - in plain text (the `text` backend), its display followed by its address between
//!   angle brackets (`Click here <https://example.com>`), or its display alone for a
//!   `\url` or an `\email`, depending on the render options
//!   [`link_addresses`](crate::stdcore::backends::text::TextRenderOptions::link_addresses)
//!   and
//!   [`link_addresses_when_shown`](crate::stdcore::backends::text::TextRenderOptions::link_addresses_when_shown);
//! - in HTML (the `html` backend), an `<a>` element whose `href` is the link's target
//!   ([hyperlinks in HTML](crate::guide::rendering#hyperlinks-in-html));
//! - in the recomposed LaTeX (the `latex-recomposed` backend), one of the LaTeX hooks
//!   (macros the style file defines and a user may redefine;
//!   [hooks](crate::stdcore::backends::latex::preamble)) `\flmHref`, `\flmUrl` and
//!   `\flmEmail` ([`latex`]).
//!
//! # The address is raw text
//!
//! The address argument is a verbatim argument delimited by `{` and `}`
//! ([`ArgStdKind::Verbatim`]): everything up to the matching `}` is a character of the
//! address, including `%`, `#`, `\`, `~`, `_` and nested braces, which are counted and
//! kept. A percent-encoded address (`https://example.com/a%20b`) is therefore written
//! as it is. In the template of a user definition the placeholders still work there,
//! so `\href{https://doi.org/#1}{#1}` puts the use site's characters into the address;
//! a literal `#` in such a template is written `##`, as everywhere in a template.
//!
//! The rules read the address as characters with [`chars_at_lowering`], the only
//! reading of content as characters at lowering, so they never write an address whose
//! characters are not known. On the recomposed LaTeX, a user definition whose template
//! holds a placeholder in an address therefore has every use site expanded rather than
//! written as a call. Content that is not characters (a command that a use site passed
//! into a definition's address, as `\doi{\emph{x}}` does for the definition above) is
//! the Error `flm.href.address-not-characters` ([`AddressNotCharacters`]), and no link
//! is made (see [`AddressNotCharacters`] for what is written instead).
//!
//! **The feature never inspects an address** except to derive its display: it checks
//! no scheme, validates nothing and normalizes nothing. Which addresses an output
//! accepts is decided by that output: the HTML backend refuses an address whose scheme
//! it does not accept when it writes the page, and the link keeps its text
//! ([`check_address`](crate::stdcore::backends::html::check_address)).
//!
//! # The display
//!
//! `\href`'s display is its text argument, lowered as content. The display of `\url`
//! and `\email` is derived from the link's target by one function, [`display`]: the
//! target without its leading `http://`, `https://` or `mailto:`, and without any
//! trailing run of `/`, `#` and `?` characters; `https://example.com/?#` shows
//! `example.com`. The link states which kind of display it has
//! ([`LinkPayload::shows_target`]), so that a backend never needs to compare a link's
//! content with its target.
//!
//! Such a display is presentation: a backend's rule may derive it in its own output's
//! language, as long as it gives the same result as [`display`]. The recomposed LaTeX
//! does so: it writes the address alone, and its LaTeX hooks derive the display in
//! LaTeX ([`latex`]).
//!
//! # One element, three commands
//!
//! The three commands are three element values of one element type, [`LINK_ELEMENT`]
//! (`flm.href.link`), each carrying an [`HrefCommand`], which says whether the
//! command takes a text argument and what it writes before the address (`mailto:` for
//! `\email`). The feature has no configuration: its three commands are fixed.
//!
//! The address is not part of the words of the text the link is in: the element's
//! [`ApartFromText`] aspect returns its address argument, so a heading whose title
//! holds a link gets its anchor from the title's own words ([`plain_text_of`]). The
//! display of a `\url`, derived at lowering, is not a node of the tree, so it is not
//! among those words either.
//!
//! # The lowering rules
//!
//! The default rule, [`RULE`] (`flm.href.link.lower.default`, [`LinkRule`]), applies
//! on every backend except `latex-recomposed`. It stages (builds into the lowered tree;
//! see [`Lowerer`]) a [`flm.pres.link`](crate::stdcore::backends::pres::LINK) with the
//! kind token [`LINK_KIND`] (`url`) whose `href` is the address as written, with `mailto:` before
//! the address of an `\email`, around the display.
//!
//! The rule on the recomposed LaTeX, [`latex::LATEX_RULE`], writes each command as a
//! LaTeX hook that the style file defines, `\flmHref{address}{text}`,
//! `\flmUrl{address}` or `\flmEmail{address}` (the last two derive the display
//! themselves), with the address escaped by the LaTeX family's escaping of an address
//! ([`escape_url`](crate::stdcore::backends::latex::escape_url)). A user redefines a
//! hook to link or typeset differently ([`latex`]).
//!
//! # Items
//!
//! - The feature: [`Href`], [`feature`], [`FEATURE`].
//! - The element: [`LINK_ELEMENT`], [`HrefCommand`], the argument names
//!   [`ADDRESS_ARG`] and [`TEXT_ARG`], and the argument role
//!   [`ADDRESS_ARG_ROLE`] (`flm.href.address`), which states that an argument
//!   holds a link's address ([`ArgDef::role`](crate::lang::mint::ArgDef::role)).
//! - The display: [`display`], [`DISPLAY_PREFIXES`], [`DISPLAY_TRAILING`].
//! - The default rule: [`RULE`], [`LinkRule`], [`rule`], [`LINK_KIND`].
//! - The diagnostic: [`AddressNotCharacters`].
//! - The recomposed LaTeX: [`latex`].

pub mod latex;

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    chars_at_lowering,
};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use crate::lang::text::plain_text_of;
use crate::lang::Flm;
use crate::model::aspects::{
    ApartFromText, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole,
};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::{ElementName, FeatureName};
use crate::stdcore::backends::lowering::lower_arg_named_or_nothing;
use crate::stdcore::backends::pres::{self, LinkPayload};

/// The feature's registered name, `href`.
pub const FEATURE: &str = "href";

/// The name of the element type of the three commands, `flm.href.link`. Each
/// command is one element value of this type, carrying an [`HrefCommand`].
pub const LINK_ELEMENT: &str = "flm.href.link";

/// The name of the default lowering rule of [`LINK_ELEMENT`],
/// `flm.href.link.lower.default`, implemented by [`LinkRule`]: the rule on every
/// backend except `latex-recomposed`, which has its own
/// ([`latex::LATEX_RULE`]).
pub const RULE: &str = "flm.href.link.lower.default";

/// The kind token written on every link this feature stages, `url`: a link to an
/// address outside the document. The HTML backend writes it as the attribute
/// `data-flm-kind="url"`.
pub const LINK_KIND: &str = "url";

/// The name of the address argument, `address`.
pub const ADDRESS_ARG: &str = "address";

/// The argument role of a link's address, `flm.href.address`: the role of
/// the address argument ([`ADDRESS_ARG`]) of `\href`, `\url` and `\email`.
pub const ADDRESS_ARG_ROLE: &str = "flm.href.address";

/// The name of the text argument of `\href`, `text`.
pub const TEXT_ARG: &str = "text";

/// The prefixes that [`display`] removes from the start of a target, the first
/// one the target begins with: `http://`, `https://`, `mailto:`.
pub const DISPLAY_PREFIXES: [&str; 3] = ["http://", "https://", "mailto:"];

/// The characters that [`display`] removes from the end of a target, in a run
/// of any length: `/`, `#` and `?`.
pub const DISPLAY_TRAILING: [char; 3] = ['/', '#', '?'];

/// Which of the three commands a node was written with: the data of each element
/// value of [`LINK_ELEMENT`].
///
/// A lowering rule reads it from a node with [`element_value`]. Its methods say
/// what the command means (whether it takes a text argument, what it writes
/// before the address), so that no rule needs to compare the name a node was
/// written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HrefCommand {
    /// `\href{address}{text}`: a link shown as the author's text.
    Href,
    /// `\url{address}`: a link shown as its address's display.
    Url,
    /// `\email{address}`: a link to `mailto:<address>`, shown as the
    /// address's display.
    Email,
}

impl HrefCommand {
    /// The three commands, in the order the feature declares them.
    pub const ALL: [HrefCommand; 3] = [HrefCommand::Href, HrefCommand::Url, HrefCommand::Email];

    /// Returns the command's name, without the backslash: `href`, `url`,
    /// `email`.
    pub fn name(self) -> &'static str {
        match self {
            HrefCommand::Href => "href",
            HrefCommand::Url => "url",
            HrefCommand::Email => "email",
        }
    }

    /// Returns whether the command takes a text argument after its address:
    /// only `\href` does. A command without one shows its target's display
    /// ([`display`]).
    pub fn has_text(self) -> bool {
        matches!(self, HrefCommand::Href)
    }

    /// Returns what the command writes before the address to make the link's
    /// target: `mailto:` for `\email`, nothing for the other two.
    pub fn prefix(self) -> &'static str {
        match self {
            HrefCommand::Email => "mailto:",
            HrefCommand::Href | HrefCommand::Url => "",
        }
    }
}

/// Returns the display of a link's target, as `\url` and `\email` show it: the
/// target without the first of [`DISPLAY_PREFIXES`] it begins with, and without
/// any trailing run of [`DISPLAY_TRAILING`] characters. This function is the only
/// place where FLM computes the display; the LaTeX hooks of the recomposed LaTeX
/// derive the same display in LaTeX from the address they receive ([`latex`]).
///
/// `https://example.com/?#` shows `example.com`; `mailto:someone@example.com`
/// shows `someone@example.com`. Nothing else of the target is changed: the
/// comparison is exact, case included, and the display may be empty
/// (`https://` shows nothing).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::href::display;
///
/// assert_eq!(display("https://example.com/?#"), "example.com");
/// assert_eq!(display("mailto:a@b.c"), "a@b.c");
/// assert_eq!(display("ftp://example.com/"), "ftp://example.com");
/// ```
pub fn display(target: &str) -> String {
    let rest = DISPLAY_PREFIXES
        .iter()
        .find_map(|prefix| target.strip_prefix(prefix))
        .unwrap_or(target);
    rest.trim_end_matches(DISPLAY_TRAILING).to_string()
}

/// Condition `flm.href.address-not-characters`, an Error: the address of a
/// `\href`, a `\url` or an `\email` holds content that is not characters (a
/// command, an environment, a special character sequence), which a use site of a
/// user definition passed into the definition's address (`\doi{\emph{x}}` for a
/// definition written `\href{https://doi.org/#1}{#1}`). It is reported at the
/// command, once each time the node is lowered.
///
/// Recovery: no link is made, and what the link would have shown is written
/// alone, in the same way for the three commands: the text of `\href`, or, for
/// `\url` and `\email`, the display ([`display`]) the link would have had,
/// computed from the words the address holds, read as the words of content are
/// read ([`plain_text_of`]). The display of an `\email` is its address alone,
/// never with `mailto:` before it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.href.address-not-characters",
    message = "the address of this \\{command} must hold only characters, not a command or \
               other markup"
)]
pub struct AddressNotCharacters {
    /// The command, without the backslash: `href`, `url` or `email`.
    pub command: String,
}

/// What an address argument holds, as the lowering rules read it.
pub(crate) enum Address {
    /// Characters; the command's prefix is not among them.
    Read(String),
    /// Content that is not characters: the words it holds, without the
    /// command's prefix, for the recovery of [`AddressNotCharacters`].
    NotCharacters(String),
}

/// Reads the address argument of `node` with [`chars_at_lowering`], the only
/// reading of content as characters at lowering: characters, or the words of
/// content that is not characters. An argument that a parse left absent after
/// reporting it reads as no characters at all.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] for an address holding a placeholder of
/// a user definition's template lowered without expansion: the plan then expands
/// the definition's use sites instead (see the [module documentation](self)).
pub(crate) fn read_address(node: &NodeRef<'_, Flm>) -> Result<Address, LowerError> {
    let Ok(Some(content)) = node.argument_content_nodes_named(ADDRESS_ARG) else {
        return Ok(Address::Read(String::new()));
    };
    Ok(match chars_at_lowering(content.iter())? {
        Ok(text) => Address::Read(text),
        Err(_) => Address::NotCharacters(plain_text_of(content.iter())),
    })
}

/// Returns the command a node of [`LINK_ELEMENT`] was written with; a structure
/// error for a node carrying no [`HrefCommand`].
pub(crate) fn command_of(node: &NodeRef<'_, Flm>) -> Result<HrefCommand, LowerError> {
    element_value::<HrefCommand>(node)
        .copied()
        .ok_or_else(|| LowerError::structure("a hyperlink node without its command"))
}

/// Reports [`AddressNotCharacters`] at `node`.
pub(crate) fn report_not_characters(
    node: &NodeRef<'_, Flm>,
    command: HrefCommand,
    lw: &mut dyn Lowerer,
) {
    lw.diagnose(Diagnostic::error(
        AddressNotCharacters { command: command.name().to_string() },
        node.span().clone(),
    ));
}

/// The address of a hyperlink is not part of the words of the text the link is
/// in: the aspect returns the address argument, [`ADDRESS_ARG`], for all three
/// commands. The text of `\href` remains part of those words.
impl ApartFromText for HrefCommand {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(ADDRESS_ARG.to_string())]
    }
}

/// Returns the element value of one command: an inline element carrying the
/// command as its data and as its [`ApartFromText`] aspect.
fn element(command: HrefCommand) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(LINK_ELEMENT)
            .docs(format!("`\\{}`: a hyperlink to an address outside the document.", command.name()))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn ApartFromText>(Arc::new(command))
            .with_value(Arc::new(command)),
    )
}

/// Returns the arguments of a command: the verbatim address, then, for `\href`,
/// the text.
fn arguments(command: HrefCommand) -> Vec<ArgDef> {
    let mut args = vec![ArgDef::named(
        ADDRESS_ARG,
        ArgStdKind::Verbatim { delimiters: Some(('{', '}')) },
    )
    .docs(
        "The address, raw text up to the matching `}`: `%`, `#`, `\\`, `~` and nested \
         braces are characters of the address.",
    )
    .role(ADDRESS_ARG_ROLE)];
    if command.has_text() {
        args.push(ArgDef::mandatory(TEXT_ARG).docs("The text shown as the link."));
    }
    args
}

// The recovery of an unreadable address, one rule for the three commands:
// see decision rationale at [§flm-dd-dr:std-href-unreadable-address].

/// The implementation of the lowering rule [`RULE`]: it stages a
/// [`flm.pres.link`](crate::stdcore::backends::pres::LINK) with the kind token
/// [`LINK_KIND`] to the address, with the command's prefix before it, around the display: the
/// text of `\href` lowered as content, or the target's [`display`] as text, in
/// which case the payload says so ([`LinkPayload::shows_target`]).
///
/// An address that is not characters is reported ([`AddressNotCharacters`]) and
/// no link is made; the display is then written alone.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinkRule;

impl Lower for LinkRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let command = command_of(&node)?;
        let (target, linked) = match read_address(&node)? {
            Address::Read(text) => (text, true),
            Address::NotCharacters(words) => {
                report_not_characters(&node, command, lw);
                (words, false)
            }
        };
        let target = format!("{}{target}", command.prefix());
        let content = shown(cx, lw, &node, command, &target)?;
        if !linked {
            return Ok(Lowering::Emit(content));
        }
        let payload = LinkPayload {
            href: target,
            kind: LINK_KIND.into(),
            doc_role: None,
            label: None,
            shows_target: !command.has_text(),
        };
        let link = lw.pres_with(cx, &ElementName::from(pres::LINK), payload, content)?;
        Ok(Lowering::Emit(vec![link]))
    }
}

/// Returns what a link shows: the text of `\href`, lowered as content, or the
/// display of `target` as one text node, or nothing where the display is
/// empty.
fn shown(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    command: HrefCommand,
    target: &str,
) -> Result<Vec<BuildId>, LowerError> {
    if command.has_text() {
        return lower_arg_named_or_nothing(cx, lw, node, TEXT_ARG);
    }
    let display = display(target);
    if display.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![lw.text(cx, &display)?])
}

/// Returns the default lowering rule [`RULE`] of [`LINK_ELEMENT`], marked as the
/// default on every backend; the `latex-recomposed` backend uses its own rule
/// ([`latex::latex_rule`]), which is marked as the default there.
pub fn rule() -> LoweringRule {
    LoweringRule::new(
        RULE,
        LINK_ELEMENT,
        LoweringDecl::new().produces(pres::LINK),
        Arc::new(LinkRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "a hyperlink lowers to `flm.pres.link` with the kind token `url` to its address, around \
         its text or its address's display",
    )
}

/// The `href` feature: it registers the three commands, their default lowering
/// rule and their rule on the recomposed LaTeX. See the [module
/// documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct Href;

impl FlmFeature for Href {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Hyperlinks").docs(
            "Links to an address outside the document: `\\href{address}{text}`, \
             `\\url{address}` and `\\email{address}`, lowering to `flm.pres.link` with the kind \
             token `url`.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        b.argument_role(
            ADDRESS_ARG_ROLE,
            "The address of a link: a URL, or an email address for `\\email`.",
        )?;
        for command in HrefCommand::ALL {
            let name = command.name();
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [name],
                    SyntaxDef::Macro { args: arguments(command) },
                    element(command),
                )
                .docs(match command {
                    HrefCommand::Href => "`\\href{address}{text}`: the text, linked to the address.",
                    HrefCommand::Url => "`\\url{address}`: the address, linked and shown as its display.",
                    HrefCommand::Email => {
                        "`\\email{address}`: the address, linked with `mailto:` before it and \
                         shown as its display."
                    }
                }),
            )?;
        }
        b.lowering_rule(rule())?;
        latex::register(b)?;
        Ok(())
    }
}

/// Returns the [`Href`] feature, ready to be listed in a package entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Href)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_display_drops_one_prefix_and_the_trailing_run() {
        assert_eq!(display("https://example.com/?#"), "example.com");
        assert_eq!(display("http://example.com/a/b//"), "example.com/a/b");
        assert_eq!(display("mailto:a@b.c"), "a@b.c");
        // One prefix only, the comparison exact.
        assert_eq!(display("https://http://x"), "http://x");
        assert_eq!(display("HTTPS://X.ORG"), "HTTPS://X.ORG");
        assert_eq!(display("https://"), "");
        assert_eq!(display("#?/"), "");
        assert_eq!(display(""), "");
        // A trailing character of the run inside the address stays.
        assert_eq!(display("x.org/?a=1#b"), "x.org/?a=1#b");
    }

    #[test]
    fn the_three_commands_say_what_they_take_and_what_they_put_before_the_address() {
        let names: Vec<&str> = HrefCommand::ALL.iter().map(|command| command.name()).collect();
        assert_eq!(names, ["href", "url", "email"]);
        assert!(HrefCommand::Href.has_text());
        assert!(!HrefCommand::Url.has_text() && !HrefCommand::Email.has_text());
        assert_eq!(HrefCommand::Email.prefix(), "mailto:");
        assert_eq!(HrefCommand::Url.prefix(), "");
    }

    #[test]
    fn every_command_takes_a_braced_verbatim_address() {
        for command in HrefCommand::ALL {
            let args = arguments(command);
            assert_eq!(args[0].name.as_deref(), Some(ADDRESS_ARG));
            let braced = ArgDef::new(ArgStdKind::Verbatim { delimiters: Some(('{', '}')) });
            assert_eq!(args[0].kind.identity(), braced.kind.identity());
            assert_eq!(args.len(), if command.has_text() { 2 } else { 1 });
        }
    }
}
