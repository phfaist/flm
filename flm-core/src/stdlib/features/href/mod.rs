//! This module defines the `href` feature ([`Href`]), which provides three commands
//! that insert a link to an address outside the document:
//!
//! | written | links to | shows |
//! |---|---|---|
//! | `\href{https://example.com}{Click here}` | `https://example.com` | `Click here` |
//! | `\url{https://example.com/}` | `https://example.com/` | `example.com` |
//! | `\email{someone@example.com}` | `mailto:someone@example.com` | `someone@example.com` |
//!
//! The first argument of each command is the *address*, as the author wrote it. The
//! *target* of a link is the address to which the link points: for `\email`, the
//! address preceded by `mailto:`, and for `\href` and `\url`, the address itself
//! ([`HrefCommand::prefix`]). A link written with `\href` shows its second argument.
//! A link written with `\url` or `\email` shows the *display* of its target, that is,
//! the target without a leading `http://`, `https://` or `mailto:`, and without
//! trailing `/`, `#` and `?` characters ([`display`]).
//!
//! The three commands can be written only in text mode. The feature has no
//! configuration. The language guide describes the commands for authors
//! ([hyperlinks](crate::guide::language_syntax#hyperlinks)).
//!
//! # The address
//!
//! The address argument is read as raw text up to the matching `}`
//! ([`ArgStdKind::Verbatim`]). Every character before this brace is part of the
//! address, including `%`, `#`, `\`, `~` and nested braces. For instance,
//! `\url{https://example.com/a%20b#top}` links to `https://example.com/a%20b#top`.
//!
//! In the template of a user definition, a placeholder in an address is replaced by
//! the corresponding argument of the call, and a literal `#` is written `##`, as
//! everywhere in a template. For instance, if the template of `\doi` is
//! `\href{https://doi.org/#1}{#1}`, the call `\doi{10.1000/xyz}` links to
//! `https://doi.org/10.1000/xyz`. The argument of the call must be plain text.
//! Otherwise, the error [`AddressNotCharacters`] is reported and no link is made.
//!
//! The feature does not validate or normalize addresses. A backend can refuse an
//! address when it writes a link. For instance, the HTML backend refuses an address
//! whose scheme is not allowed by the render option
//! [`HtmlAllowedUrlSchemes`](crate::stdlib::backends::html::HtmlAllowedUrlSchemes).
//! The HTML backend then writes the link without an `href` attribute, and reports
//! the error `flm.html.refused-address`.
//!
//! # The element and the lowering rules
//!
//! The three commands share one element type, [`LINK_ELEMENT`] (`flm.href.link`).
//! Each command has its own element value, whose data is an [`HrefCommand`].
//!
//! The feature registers two lowering rules for this element type:
//!
//! - The default rule [`RULE`] ([`LinkRule`]) is used on every backend except
//!   `latex-recomposed`. It lowers a link to a
//!   [`flm.pres.link`](crate::stdlib::backends::pres::LINK) node of kind
//!   [`LINK_KIND`] (`url`), which points to the target of the link. The node
//!   contains the text argument of `\href`, or the display of the target for `\url`
//!   and `\email`.
//! - The rule [`latex::LATEX_RULE`] is used on the `latex-recomposed` backend. It
//!   writes each link as a call of a *LaTeX hook*, that is, a macro that is defined
//!   in the style file and that a user can redefine: `\flmHref{address}{text}`,
//!   `\flmUrl{address}` or `\flmEmail{address}`. The hooks `\flmUrl` and `\flmEmail`
//!   compute the display in LaTeX, with the same result as [`display`]. See the
//!   module [`latex`].
//!
//! Each backend writes a `flm.pres.link` node in its own format. For instance, the
//! HTML backend writes an `<a>` element whose `href` attribute is the target
//! ([hyperlinks in HTML](crate::guide::rendering#hyperlinks-in-html)). The `text`
//! backend writes what the link shows, followed by the target between angle
//! brackets, as in `Click here <https://example.com>`. By default, the `text`
//! backend omits the target after a link written with `\url` or `\email`, since such
//! a link already shows its address. The render options
//! [`link_addresses`](crate::stdlib::backends::text::TextRenderOptions::link_addresses)
//! and
//! [`link_addresses_when_shown`](crate::stdlib::backends::text::TextRenderOptions::link_addresses_when_shown)
//! change this behavior. The `text` backend recognizes such a link by the field
//! [`LinkPayload::shows_target`], which the default rule sets for `\url` and `\email`.

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
use crate::stdlib::backends::lowering::lower_arg_named_or_nothing;
use crate::stdlib::backends::pres::{self, LinkPayload};

/// The feature's registered name, `href`.
pub const FEATURE: &str = "href";

/// The name of the element type of `\href`, `\url` and `\email`, `flm.href.link`.
/// Each of the three commands has its own element value of this type, whose data
/// is an [`HrefCommand`].
pub const LINK_ELEMENT: &str = "flm.href.link";

/// The name of the default lowering rule of [`LINK_ELEMENT`],
/// `flm.href.link.lower.default`. The rule is implemented by [`LinkRule`], and is
/// used on every backend except `latex-recomposed`. On the `latex-recomposed`
/// backend, the rule [`latex::LATEX_RULE`] is used instead.
pub const RULE: &str = "flm.href.link.lower.default";

/// The link kind of the links that the default rule [`RULE`] builds, `url`
/// ([`LinkPayload::kind`]). This kind marks a link to an address outside the
/// document. The HTML backend writes it as the attribute `data-flm-kind="url"`.
pub const LINK_KIND: &str = "url";

/// The name of the address argument, `address`.
pub const ADDRESS_ARG: &str = "address";

/// The name of the argument role `flm.href.address`, which marks an argument that
/// contains the address of a link. The address argument [`ADDRESS_ARG`] of `\href`,
/// `\url` and `\email` has this role
/// ([`ArgDef::role`](crate::lang::mint::ArgDef::role)).
pub const ADDRESS_ARG_ROLE: &str = "flm.href.address";

/// The name of the text argument of `\href`, `text`.
pub const TEXT_ARG: &str = "text";

/// The prefixes that [`display`] removes from the start of a link target:
/// `http://`, `https://` and `mailto:`. At most one prefix is removed.
pub const DISPLAY_PREFIXES: [&str; 3] = ["http://", "https://", "mailto:"];

/// The characters that [`display`] removes from the end of a link target: `/`,
/// `#` and `?`. All of these characters at the end of the target are removed, in
/// any number and in any order.
pub const DISPLAY_TRAILING: [char; 3] = ['/', '#', '?'];

/// The command with which a link is written: `\href`, `\url` or `\email`.
///
/// An `HrefCommand` is the data of each element value of [`LINK_ELEMENT`]. A
/// lowering rule reads it from a node with [`element_value`]. The methods of
/// `HrefCommand` return the properties of the command that a lowering rule needs:
/// whether the command has a text argument ([`has_text`](Self::has_text)), and the
/// prefix of the link target ([`prefix`](Self::prefix)). A lowering rule therefore
/// does not need to compare the names of the commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HrefCommand {
    /// The command `\href{address}{text}`. The link shows `text`.
    Href,
    /// The command `\url{address}`. The link shows the display of the address
    /// ([`display`]).
    Url,
    /// The command `\email{address}`. The link points to `mailto:` followed by
    /// the address, and shows the address.
    Email,
}

impl HrefCommand {
    /// The three commands, in the order `\href`, `\url`, `\email`.
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

    /// Returns whether the command has a text argument after the address. Only
    /// `\href` has one. The link of a command without a text argument shows the
    /// display of its target ([`display`]).
    pub fn has_text(self) -> bool {
        matches!(self, HrefCommand::Href)
    }

    /// Returns the prefix that is placed before the address to form the link
    /// target: `mailto:` for `\email`, and an empty string for `\href` and
    /// `\url`.
    pub fn prefix(self) -> &'static str {
        match self {
            HrefCommand::Email => "mailto:",
            HrefCommand::Href | HrefCommand::Url => "",
        }
    }
}

/// Returns the display of the link target `target`, that is, the text that a
/// link written with `\url` or `\email` shows.
///
/// The display is the target without a leading prefix from [`DISPLAY_PREFIXES`]
/// (`http://`, `https://` or `mailto:`), and without the trailing characters from
/// [`DISPLAY_TRAILING`] (`/`, `#` and `?`). The prefixes are compared exactly,
/// including the letter case. The display can be empty, as for `https://`.
///
/// The LaTeX hooks of the `latex-recomposed` backend compute the same display in
/// LaTeX ([`latex`]).
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::href::display;
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

/// The error that is reported when the address of `\href`, `\url` or `\email` is
/// not plain text (`flm.href.address-not-characters`).
///
/// The address that an author writes in one of these commands is always plain
/// text, because the address argument is read as raw text. Other content gets
/// into an address through a user definition whose template places one of its
/// arguments in the address. A call of such a definition must pass plain text in
/// this argument. For instance, if the template of `\doi` is
/// `\href{https://doi.org/#1}{#1}`, the call can be `\doi{10.1000/xyz}`.
///
/// Reported by the lowering rule of the command when it is invoked. No link is
/// made. `\href` expands to its text argument. `\url` and `\email` expand to the
/// display ([`display`]) of the address, computed from the text of the address
/// without its markup.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.href.address-not-characters",
    message = "Invalid address in ‘\\{command}’: the address must be plain text"
)]
pub struct AddressNotCharacters {
    /// The name of the command, without the backslash: `href`, `url` or `email`.
    pub command: String,
}

/// The address of a link, as returned by [`read_address`].
pub(crate) enum Address {
    /// The address is plain text. The string contains its characters, without
    /// the prefix of the command ([`HrefCommand::prefix`]).
    Read(String),
    /// The address contains content that is not plain text. The string contains
    /// the text of the address without its markup ([`plain_text_of`]), and
    /// without the prefix of the command. After reporting
    /// [`AddressNotCharacters`], the lowering rules compute the display of a
    /// `\url` or an `\email` from this text.
    NotCharacters(String),
}

/// Reads the address argument of the link node `node` with
/// [`chars_at_lowering`].
///
/// Returns [`Address::Read`] when the address is plain text, and
/// [`Address::NotCharacters`] otherwise. When the node has no address argument,
/// the address is empty.
///
/// # Errors
///
/// Returns [`LowerError::PlaceholderCharacters`] when the address contains a
/// placeholder of a user definition's template. This happens only when the
/// template is lowered without being expanded, which is the case on the
/// `latex-recomposed` backend for a definition that is written as a LaTeX
/// definition. The user-definitions feature then expands every use site of the
/// definition ([`LowerError::PlaceholderCharacters`]).
pub(crate) fn read_address(node: &NodeRef<'_, Flm>) -> Result<Address, LowerError> {
    let Ok(Some(content)) = node.argument_content_nodes_named(ADDRESS_ARG) else {
        return Ok(Address::Read(String::new()));
    };
    Ok(match chars_at_lowering(content.iter())? {
        Ok(text) => Address::Read(text),
        Err(_) => Address::NotCharacters(plain_text_of(content.iter())),
    })
}

/// Returns the command with which the link node `node` was written, from the data
/// of the element of the node.
///
/// # Errors
///
/// Returns a [`LowerError::Structure`] error when the data of the element is not
/// an [`HrefCommand`].
pub(crate) fn command_of(node: &NodeRef<'_, Flm>) -> Result<HrefCommand, LowerError> {
    element_value::<HrefCommand>(node)
        .copied()
        .ok_or_else(|| {
            LowerError::structure("missing ‘HrefCommand’ data on the element of the node")
        })
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

/// Excludes the address argument [`ADDRESS_ARG`] of a link from the plain text of
/// the content around the link ([`plain_text_of`]).
///
/// For instance, when the identifier of a heading is computed from its title, the
/// address of a link in the title is left out. The text argument of `\href` is
/// part of the plain text. The display of a `\url` or an `\email` is computed
/// only when the link is lowered, so it is not part of the plain text either.
impl ApartFromText for HrefCommand {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(ADDRESS_ARG.to_string())]
    }
}

/// Returns the element value of the command `command`. The element is inline
/// content, its data is `command`, and its [`ApartFromText`] aspect excludes the
/// address from the plain text.
fn element(command: HrefCommand) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(LINK_ELEMENT)
            .docs(match command {
                HrefCommand::Href => {
                    "A link to an address outside the document, shown as the text written for it."
                }
                HrefCommand::Url => {
                    "A link to an address outside the document, shown as the address."
                }
                HrefCommand::Email => "A link to an email address, shown as the address.",
            })
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn ApartFromText>(Arc::new(command))
            .with_value(Arc::new(command)),
    )
}

/// Returns the arguments of a command: the verbatim address, then, for `\href`,
/// the text.
fn arguments(command: HrefCommand) -> Vec<ArgDef> {
    let address = match command {
        HrefCommand::Email => "The email address, as in `someone@example.com`.",
        HrefCommand::Href | HrefCommand::Url => {
            "The address of the link, as in `https://example.com/a%20b`."
        }
    };
    let mut args = vec![ArgDef::named(
        ADDRESS_ARG,
        ArgStdKind::Verbatim { delimiters: Some(('{', '}')) },
    )
    .docs(format!(
        "{address} The address is raw text up to the matching `}}`: the characters `%`, `#`, \
         `\\`, `~` and nested braces are part of the address."
    ))
    .role(ADDRESS_ARG_ROLE)];
    if command.has_text() {
        args.push(
            ArgDef::mandatory(TEXT_ARG).docs("The text that the link shows, as in `Click here`."),
        );
    }
    args
}

// The recovery of an unreadable address is one rule for the three commands:
// what the link would show is shown alone, so that a reader still sees what the
// author meant to show.

/// The implementation of the default lowering rule [`RULE`] of `\href`, `\url`
/// and `\email`.
///
/// The rule lowers a link to a
/// [`flm.pres.link`](crate::stdlib::backends::pres::LINK) node of kind
/// [`LINK_KIND`] (`url`), whose field [`LinkPayload::href`] is the link target.
/// For `\href`, the node contains the text argument, lowered as content. For
/// `\url` and `\email`, the node contains a text node with the display of the
/// target ([`display`]), and the rule sets the field
/// [`LinkPayload::shows_target`].
///
/// When the address is not plain text, the rule reports [`AddressNotCharacters`]
/// and builds no link node. It then returns only the content that the link would
/// have contained.
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

/// Lowers the content of a link, and returns the lowered nodes. For `\href`, the
/// content is the text argument of `node`. For `\url` and `\email`, the content is
/// one text node with the display of `target`, or no node when the display is
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

/// Returns the default lowering rule [`RULE`] of [`LINK_ELEMENT`], implemented by
/// [`LinkRule`].
///
/// The rule is marked as the default on every backend
/// ([`DefaultMark::Everywhere`]). On the `latex-recomposed` backend, the rule
/// returned by [`latex::latex_rule`] is selected instead, because it is marked as
/// the default for this backend.
pub fn rule() -> LoweringRule {
    LoweringRule::new(
        RULE,
        LINK_ELEMENT,
        LoweringDecl::new().produces(pres::LINK),
        Arc::new(LinkRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "Lowers a link to a `flm.pres.link` node of kind `url`, which points to the address of \
         the link, preceded by `mailto:` for `\\email`. The node contains the text argument of \
         `\\href`, or, for `\\url` and `\\email`, the address as these commands show it.",
    )
}

/// The `href` feature, which provides the commands `\href`, `\url` and `\email`
/// for links to addresses outside the document. See the [module
/// documentation](self).
///
/// The feature registers the three commands, the argument role
/// [`ADDRESS_ARG_ROLE`], the default lowering rule [`RULE`], and the lowering rule
/// [`latex::LATEX_RULE`] of the `latex-recomposed` backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct Href;

impl FlmFeature for Href {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Hyperlinks").docs(
            "Links to addresses outside the document, inserted with `\\href{address}{text}`, \
             `\\url{address}` and `\\email{address}`.",
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
                    HrefCommand::Href => "Inserts a link to `address`, shown as `text`.",
                    HrefCommand::Url => {
                        "Inserts a link to an address, shown as the address without a leading \
                         `http://`, `https://` or `mailto:` and without trailing `/`, `#` and `?` \
                         characters."
                    }
                    HrefCommand::Email => {
                        "Inserts a link to an email address, shown as the address. The link points \
                         to `mailto:` followed by the address."
                    }
                }),
            )?;
        }
        b.element_type_text(
            LINK_ELEMENT,
            "A link to an address outside the document, such as a web page or an email \
             address. The link shows either the text written for it or the address.",
        )?;
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
