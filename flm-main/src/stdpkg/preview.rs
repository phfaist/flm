//! The feature `preview` of the package `stdmain`: it declares the script and the stylesheet
//! of the live preview's page, as a script definition and a style definition, which the
//! HTML backend writes into a page that requires them.
//!
//! While `flm watch` runs, the page shown in the browser carries a script of the host's,
//! the *page script*, which keeps the page in step with the document: it listens to the
//! preview's server and brings the page up to date after each build ([`crate::watch`]). A
//! style definition or a script definition is a stylesheet or a script that a feature's
//! output depends on, recorded in the registry under an identifier, which the output then
//! requires by identifier ([`StyleDef`], [`ScriptDef`]). The feature `preview` declares
//! the page script ([`SCRIPT`], `flm.preview.script`) and the stylesheet of what the
//! script adds to the page ([`STYLE`], `flm.preview.style`).
//!
//! The preview's builds require both definitions, together with the HTML backend's
//! stylesheet for the marks of errors, through the HTML backend's render option
//! [`required_definitions`](flm_core::stdcore::backends::html::HtmlRenderOptions::required_definitions)
//! ([`crate::watch::build_settings`]). A build that does not require them, such as one of
//! `flm render` or `flm check`, writes nothing of the preview into its page. The feature
//! is nevertheless registered under every command, because the registry is the same for
//! every command.
//!
//! The feature declares no settings and registers no stage. Like every feature of `stdmain`,
//! it can be switched off in the settings (`preview: false`); the preview's builds then
//! require definitions that no feature declares, which the HTML backend reports.

use flm_core::core::registry::{BuildError, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, ScriptDef, StyleDef};
use flm_core::model::names::Name;

/// The name under which the feature is registered, `preview`.
pub const FEATURE: &str = "preview";

/// The identifier of the script definition of the page script, `flm.preview.script`.
pub const SCRIPT: &str = "flm.preview.script";

/// The identifier of the style definition of the page script's stylesheet,
/// `flm.preview.style`.
pub const STYLE: &str = "flm.preview.style";

/// The text of the page script: plain JavaScript that current browsers run without a
/// build step, held in one function that defines no global name of its own. Its first
/// line is a comment holding [`SCRIPT`].
///
/// The text joins files of the folder `watch/page/` of this crate's sources, in this
/// order: the beginning of the function; the library `morphdom` (kept unmodified under
/// `vendor/morphdom/`, beside its license); the update of the page; the problems (their
/// marks in the page and their list); the toolbar; the names through which the form
/// library hands over its classes (`json-editor-begin.js`); the form library
/// `@json-editor/json-editor` (kept unmodified under `vendor/json-editor/`, beside its
/// license), which the toolbar's tool *Settings* uses; the tool *Settings*; the gestures
/// (opening the place of an element in the editor, copying a reference to it); the
/// connection to the server; and the end of the function. The [`crate::watch`] module's
/// documentation and the command-line guide's section on the live preview describe what
/// the script does.
///
/// **Changes to the page's global objects.** The polyfill library `core-js`, which
/// `@json-editor/json-editor` bundles, makes four changes when the script loads, on every
/// preview page: it adds `__core-js_shared__` to `window`, adds `useSetter` and
/// `useSimple` to `Symbol`, and replaces `Function.prototype.toString` by a wrapper that
/// returns the original text for every function other than the library's own.
pub const SCRIPT_TEXT: &str = concat!(
    include_str!("../watch/page/begin.js"),
    include_str!("../watch/page/vendor/morphdom/morphdom-umd.js"),
    include_str!("../watch/page/regions.js"),
    include_str!("../watch/page/problems.js"),
    include_str!("../watch/page/toolbar.js"),
    include_str!("../watch/page/json-editor-begin.js"),
    include_str!("../watch/page/vendor/json-editor/jsoneditor.js"),
    include_str!("../watch/page/settings.js"),
    include_str!("../watch/page/gestures.js"),
    include_str!("../watch/page/connection.js"),
    include_str!("../watch/page/end.js"),
);

/// The text of the stylesheet of what the page script adds to the page (the toolbar, the
/// marks of problems and the server's waiting page), from the file `watch/page/preview.css`
/// of this crate's sources.
/// Its first line is a comment holding [`STYLE`].
pub const STYLE_TEXT: &str = include_str!("../watch/page/preview.css");

/// The feature `preview`, which declares the script and the stylesheet of the live
/// preview's page: see the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct PreviewFeature;

impl FlmFeature for PreviewFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "The live preview's page")
            .docs(
                "the script that keeps a page in step with its document while `flm watch` runs, and the \
                 stylesheet of what the script adds to the page",
            )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.style_def(StyleDef {
            id: Name::from(STYLE),
            content: vec![DefContent::Inline(String::from(STYLE_TEXT))],
            docs: String::from("the stylesheet of what the live preview's page script adds to the page"),
        })?;
        b.script_def(ScriptDef {
            id: Name::from(SCRIPT),
            content: vec![DefContent::Inline(String::from(SCRIPT_TEXT))],
            docs: String::from("the live preview's page script, which keeps the page in step with the document"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_text_begins_with_its_identifier_and_cannot_end_its_element_early() {
        assert_eq!(SCRIPT_TEXT.lines().next(), Some(format!("/* {SCRIPT} */").as_str()));
        assert_eq!(STYLE_TEXT.lines().next(), Some(format!("/* {STYLE} */").as_str()));
        assert!(!SCRIPT_TEXT.to_ascii_lowercase().contains("</script"));
        assert!(!STYLE_TEXT.to_ascii_lowercase().contains("</style"));
    }

    #[test]
    fn the_script_follows_the_contract_for_a_page_updated_in_place() {
        use flm_core::stdcore::backends::html::page::{ORIGINAL_HTML_ATTRIBUTE, REGION_UPDATED_EVENT};
        assert!(SCRIPT_TEXT.contains(&format!("'{REGION_UPDATED_EVENT}'")));
        assert!(SCRIPT_TEXT.contains(&format!("'{ORIGINAL_HTML_ATTRIBUTE}'")));
    }
}
