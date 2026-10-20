//! The hyperlinks on the `latex-recomposed` backend: each command of the
//! [`href`](super) feature written back as a LaTeX hook, a macro that the recomposed
//! LaTeX writes and the style file defines with a plain default, which a user
//! redefines in the document's preamble
//! ([`latex::preamble`](crate::stdcore::backends::latex::preamble); see [the LaTeX
//! hooks](crate::guide::rendering#the-latex-hooks)).
//!
//! | written | LaTeX |
//! |---|---|
//! | `\href{https://example.com}{Example link}` | `\flmHref{https://example.com}{Example link}` |
//! | `\url{https://example.com/}` | `\flmUrl{https://example.com/}` |
//! | `\email{a@b.com}` | `\flmEmail{a@b.com}` |
//! | `\url{https://x.org/a%20b#c}` | `\flmUrl{https://x.org/a\%20b\#c}` |
//!
//! # The three hooks
//!
//! The rule [`LATEX_RULE`] writes one hook per command. Each hook takes ordinary LaTeX
//! arguments (no argument is read with changed character codes), so that a hook still
//! works in a *moving argument*, such as the title of a heading that LaTeX writes to
//! the table of contents and reads back:
//!
//! - `\flmHref{<address>}{<text>}`, like the `\href` of the `hyperref` package, the
//!   text lowered as content through the ordinary lowering rules;
//! - `\flmUrl{<address>}`, linked through `\href` and showing the address's display,
//!   which the hook derives itself (below), typeset with the `\nolinkurl` of
//!   `hyperref`, the way `\url` typesets an address: in `\UrlFont`, with the line
//!   breaks of the `url` package;
//! - `\flmEmail{<address>}`, linked through `\href` with `mailto:` before the address,
//!   and showing the display of that target (the address without `mailto:`), derived
//!   and typeset the same way.
//!
//! The address is written through the LaTeX family's escaping of an address,
//! [`escape_url`]: `#`, `%` and `\` each preceded by a backslash, every other
//! character as it is. The address is read as characters
//! ([`chars_at_lowering`](crate::core::lowering::chars_at_lowering)), so a user
//! definition whose template holds a placeholder inside an address
//! (`\href{https://doi.org/#1}{#1}`, `\url{#1}`) is not written back as a LaTeX
//! definition: each of its use sites is expanded, and the rule writes the address the
//! use site supplies.
//!
//! # The display, derived in LaTeX
//!
//! What a `\url` or an `\email` shows is presentation, so on the recomposed LaTeX the
//! hook derives it instead of receiving it: the rule writes the address alone, and the
//! internal macro `\flm@href@display{<address>}` of the feature's definition piece
//! computes the display from it, with the same result as the feature's [`display`]:
//! the escaping backslashes removed, then one leading `http://`, `https://` or
//! `mailto:`, then the trailing run of `/`, `#` and `?`. So
//! `\flmUrl{https://example.com/?\#}` shows `example.com`, as the HTML page does, and
//! a LaTeX author who redefines `\flmUrl` receives the address and chooses what to
//! show. The internal macro is not meant to be redefined: a user redefines the hooks.
//!
//! # The definition piece in the style file
//!
//! A *definition piece* is a unit of LaTeX definitions that a feature registers under
//! an identifier `flm.latex.def.<name>`, which a lowering rule requires and the style
//! file then holds ([`LatexPreambleDef`]). The three hooks are defined by the
//! feature's definition piece ([`definition_piece`], `flm.latex.def.href`), each with
//! `\ProvideDocumentCommand`, so that a definition the user's document class or
//! preamble made first takes precedence. The piece requests the `hyperref` package,
//! because the hooks' defaults are that package's own macros `\href` and `\nolinkurl`.
//! A recomposed document that holds a hyperlink therefore loads `hyperref`, and every
//! reference written through the LaTeX hook `\flmHyperref` (a reference that displays
//! the author's own words) then becomes a link as well. A document that loads
//! `hyperref` itself, with options of its own, after the style file, must list the
//! package in the render option
//! [`skip_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::skip_packages),
//! which removes the request; otherwise LaTeX stops with an option clash. The piece
//! also says what each hook contributes to a PDF bookmark (its text or its display
//! alone), at the beginning of the document and only where `hyperref` is loaded, so
//! that this works whether the package was loaded before the style file or after it.
//!
//! An address that is not characters is reported
//! ([`AddressNotCharacters`](super::AddressNotCharacters)), as on every output, and
//! what the link would have shown is written alone.
//!
//! # Registration without the LaTeX features
//!
//! The feature registers the rule with a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of each name it uses that the LaTeX
//! features own: the backend `latex-recomposed`, the macro construct
//! `flm.pres.latex.macro`, the raw construct `flm.pres.latex.raw` (a raw node holds
//! LaTeX text written as it is) and the service slot of the definition pieces. A
//! registry without the LaTeX features therefore still builds, with the rule set aside
//! (excluded from rule selection).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexMacroPayload, LatexPreambleDef, LatexRawPayload, escape_url, packages,
    preamble,
};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::lower_arg_named_or_nothing;

use super::{
    Address, HrefCommand, LINK_ELEMENT, TEXT_ARG, command_of, display, read_address,
    report_not_characters,
};

/// The name of the lowering rule of [`LINK_ELEMENT`] on the `latex-recomposed`
/// backend, `flm.href.link.lower.latex-recomposed`, implemented by
/// [`LinkLatexRule`].
pub const LATEX_RULE: &str = "flm.href.link.lower.latex-recomposed";

/// The name of the feature's definition piece, `href`; its identifier is
/// `flm.latex.def.href` ([`PIECE`]).
pub const PIECE_NAME: &str = "href";

/// The identifier of the feature's definition piece, `flm.latex.def.href`
/// ([`definition_piece`]).
pub const PIECE: &str = "flm.latex.def.href";

/// The name of the LaTeX hook that `\href` is written as, `flmHref`.
pub const HREF_COMMAND: &str = "flmHref";

/// The name of the LaTeX hook that `\url` is written as, `flmUrl`.
pub const URL_COMMAND: &str = "flmUrl";

/// The name of the LaTeX hook that `\email` is written as, `flmEmail`.
pub const EMAIL_COMMAND: &str = "flmEmail";

/// Returns the name of the LaTeX hook that `command` is written as.
pub fn hook_of(command: HrefCommand) -> &'static str {
    match command {
        HrefCommand::Href => HREF_COMMAND,
        HrefCommand::Url => URL_COMMAND,
        HrefCommand::Email => EMAIL_COMMAND,
    }
}

/// Returns the feature's definition piece, `flm.latex.def.href`: the three LaTeX
/// hooks `\flmHref{<address>}{<text>}`, `\flmUrl{<address>}` and
/// `\flmEmail{<address>}`, each defined with `\ProvideDocumentCommand` over the
/// `\href` of `hyperref` (the display of the last two derived by the piece's
/// internal macro, with the same result as [`display`], and typeset with
/// `\nolinkurl`), and what each contributes to a PDF bookmark: its text or its
/// display alone.
///
/// The piece requests the `hyperref` package ([`packages::HYPERREF`]), whose
/// macros the hooks' defaults use; a document that loads that package itself
/// lists it in the render option `skip_packages` (see the [module
/// documentation](self)).
///
/// Redefine a hook, before loading the style file or after it, to link or to
/// typeset that command's links another way.
pub fn definition_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% Links to an address outside the document, one hook per FLM command:
% \\flmHref{<address>}{<text>}: <text>, linked to <address>, as \\href does.
% \\flmUrl{<address>}: the display of <address>, linked to <address> and
%   typeset the way \\url typesets an address.
% \\flmEmail{<address>}: the display of mailto:<address> -- the address --
%   linked to mailto:<address> and typeset the same way.
% The display is what the other outputs of FLM show of such a link: its target
% without one leading http://, https:// or mailto: and without the trailing
% run of the characters / # ? -- https://example.com/?# shows example.com.
% Every argument is an ordinary one: in <address> the characters # % \\ are
% written \\# \\% \\\\. Redefine a hook to link or typeset differently.
%
% The display is derived by \\flm@href@display{<address>}, written in expl3,
% LaTeX's programming layer, which is part of the format and needs no package;
% its names read only between \\ExplSyntaxOn and \\ExplSyntaxOff. It reads the
% address as a string of characters (\\tl_to_str:n), drops the backslash of
% each \\# \\% \\\\, then the prefix, then the trailing run, one character at a
% time. It works by expansion alone and leaves the display and nothing else,
% so that it serves where a hook is typeset and where hyperref builds a PDF
% bookmark by expansion. The macros with @ in their names are internals, not a
% customization point: redefine a hook, not its parts.
\\ExplSyntaxOn
\\def\\flm@href@display#1{\\exp_args:Ne \\flm@href@trim
  {\\exp_args:Ne \\flm@href@unprefix
    {\\exp_args:Ne \\flm@href@unescape {\\tl_to_str:n {#1}}}}}
% Every character, a backslash dropped and the character after it kept.
\\def\\flm@href@unescape#1{\\str_if_empty:nF {#1}
  {\\str_if_eq:eeTF {\\str_head:n {#1}} {\\c_backslash_str}
    {\\exp_args:Ne \\flm@href@unescape@one {\\str_tail:n {#1}}}
    {\\flm@href@unescape@one {#1}}}}
\\def\\flm@href@unescape@one#1{\\str_head:n {#1}
  \\exp_args:Ne \\flm@href@unescape {\\str_tail:n {#1}}}
% The first of the three prefixes the string begins with, taken off.
\\def\\flm@href@unprefix#1{\\flm@href@unprefix@try {#1} {http://}
  {\\flm@href@unprefix@try {#1} {https://}
    {\\flm@href@unprefix@try {#1} {mailto:} {#1}}}}
\\def\\flm@href@unprefix@try#1#2#3{\\str_if_eq:eeTF
  {\\str_range:nnn {#1} {1} {\\str_count:n {#2}}} {#2}
  {\\str_range:nnn {#1} {\\str_count:n {#2} + 1} {-1}} {#3}}
% The last character taken off while it is one of / # ?.
\\def\\flm@href@trim#1{\\exp_args:Ne \\flm@href@trim@last {\\str_item:nn {#1} {-1}} {#1}}
\\def\\flm@href@trim@last#1#2{\\bool_lazy_any:nTF
  {{\\str_if_eq_p:nn {#1} {/}} {\\str_if_eq_p:ee {#1} {\\c_hash_str}} {\\str_if_eq_p:nn {#1} {?}}}
  {\\exp_args:Ne \\flm@href@trim {\\str_range:nnn {#2} {1} {-2}}} {#2}}
% The display of <target>, typeset by \\nolinkurl.
\\def\\flm@href@show#1{\\exp_args:Ne \\nolinkurl {\\flm@href@display {#1}}}
\\ExplSyntaxOff
\\ProvideDocumentCommand\\flmHref{m m}{\\href{#1}{#2}}
\\ProvideDocumentCommand\\flmUrl{m}{\\href{#1}{\\flm@href@show{#1}}}
\\ProvideDocumentCommand\\flmEmail{m}{\\href{mailto:#1}{\\flm@href@show{mailto:#1}}}
% What such a link contributes to a PDF bookmark: its text or its display
% alone. The hook runs when the document begins, so that it also holds where
% the document loads hyperref itself, after this file, having named it in the
% render option skip_packages; it does nothing where hyperref is absent.
\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands
  \\pdfstringdefDisableCommands{%
    \\def\\flmHref#1#2{#2}%
    \\def\\flmUrl#1{\\flm@href@display{#1}}%
    \\def\\flmEmail#1{\\flm@href@display{mailto:#1}}%
  }\\fi}
",
        "`\\flmHref{address}{text}`, `\\flmUrl{address}` and `\\flmEmail{address}`: the LaTeX \
         hooks of the three hyperlink commands, linked through `hyperref`, the display of the last \
         two derived in LaTeX",
    )
    .requires([packages::HYPERREF])
}

/// The implementation of the lowering rule [`LATEX_RULE`]: it stages the
/// command's LaTeX hook ([`hook_of`]) with the address, escaped with
/// [`escape_url`], as its first argument and, for `\href`, the text lowered as
/// content as its second, and requires the definition piece [`PIECE`]. The
/// display of a `\url` or an `\email` is not written: the hook derives it. See
/// the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct LinkLatexRule;

impl Lower for LinkLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let command = command_of(&node)?;
        let address = match read_address(&node)? {
            Address::Read(text) => text,
            Address::NotCharacters(words) => {
                report_not_characters(&node, command, lw);
                return Ok(Lowering::Emit(unlinked(cx, lw, &node, command, &words)?));
            }
        };
        // The address alone: the display of a `\url` or an `\email` is the
        // hook's to derive, in LaTeX. See decision rationale at
        // [§flm-dd-dr:rend-link-display-presentation]
        let mut slots = vec![escaped(cx, lw, &address)?];
        if command.has_text() {
            slots.push(lower_arg_named_or_nothing(cx, lw, &node, TEXT_ARG)?);
        }
        lw.require(&Name::from(PIECE));
        let shapes = vec![LatexArgShape::Mandatory; slots.len()];
        let hook = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            LatexMacroPayload::new(hook_of(command), shapes),
            slots.into_iter().map(|slot| (String::from(ARG_SLOT), slot)).collect(),
        )?;
        Ok(Lowering::Emit(vec![hook]))
    }
}

/// Stages `text`, escaped with [`escape_url`], as one raw node
/// (`flm.pres.latex.raw`, LaTeX text written as it is); nothing where `text` is
/// empty.
fn escaped(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    text: &str,
) -> Result<Vec<BuildId>, LowerError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let raw = LatexRawPayload::new(escape_url(text));
    Ok(vec![lw.pres_with(cx, &ElementName::from(RAW), raw, Vec::new())?])
}

/// Stages what a link whose address is not characters shows, alone: the text of
/// `\href` lowered as content, or the display of the address's words as document
/// text.
fn unlinked(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    command: HrefCommand,
    words: &str,
) -> Result<Vec<BuildId>, LowerError> {
    if command.has_text() {
        return lower_arg_named_or_nothing(cx, lw, node, TEXT_ARG);
    }
    let shown = display(&format!("{}{words}", command.prefix()));
    if shown.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![lw.text(cx, &shown)?])
}

/// Returns the lowering rule [`LATEX_RULE`], the default on the
/// `latex-recomposed` backend only.
pub fn latex_rule() -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        LINK_ELEMENT,
        LoweringDecl::new().produces(MACRO).produces(RAW),
        Arc::new(LinkLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a hyperlink lowers to its LaTeX hook, `\\flmHref`, `\\flmUrl` or `\\flmEmail`, over its \
         escaped address and, for `\\href`, its text",
    )
}

/// Registers the rule [`LATEX_RULE`] and contributes the definition piece to the
/// service slot of the definition pieces ([`preamble::SLOT`]), with peer
/// declarations ([`PeerName`](crate::core::registry::PeerName)) of the names they
/// use that the LaTeX features own (the backend, the macro construct and the raw
/// construct), so that the feature builds in a registry without the LaTeX
/// features, with its rule set aside (excluded from rule selection).
///
/// # Errors
///
/// [`BuildError::ContributionAfterAssembly`] when called from a
/// [`Finalizer`](crate::core::registry::Finalizer), the second registration round, in
/// which a feature may no longer contribute to a service slot
/// ([`FeatureBuilder`]).
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rule of a hyperlink is the \
         default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages for a LaTeX \
         hook",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct (LaTeX text written as it is), in which the rule \
         writes an escaped address",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    b.lowering_rule(latex_rule())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_is_named_as_its_identifier_says_and_requests_hyperref() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), PIECE);
        assert_eq!(piece.requires, [Name::from(packages::HYPERREF)]);
        for hook in [HREF_COMMAND, URL_COMMAND, EMAIL_COMMAND] {
            assert!(piece.content.contains(&format!("\\ProvideDocumentCommand\\{hook}")));
        }
        // The bookmark block is asked when the document begins, so that it
        // holds where the document loads `hyperref` itself, after the style
        // file, having skipped the request.
        assert!(
            piece.content.contains("\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands"),
            "{}",
            piece.content
        );
    }

    #[test]
    fn the_rule_is_the_default_on_the_recomposed_backend_only() {
        let rule = latex_rule();
        assert_eq!(rule.element, LINK_ELEMENT);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
    }
}
