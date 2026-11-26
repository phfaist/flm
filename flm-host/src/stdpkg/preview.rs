//! The feature `preview` of the package `std`: it declares the script and the stylesheet
//! of the live preview's page, as a script definition and a style definition, which the
//! HTML backend writes into a page that requires them.
//!
//! While `flm watch` runs, the page shown in the browser carries a script of the host's,
//! the *page script*, which keeps the page in step with the document: it listens to the
//! preview's server and brings the page up to date after each build ([`crate::watch`]). A
//! style definition or a script definition is a stylesheet or a script that a feature's
//! output depends on, recorded in the registry under an identifier, which the output then
//! requires by identifier ([`StyleDef`], [`ScriptDef`]). The feature `preview` declares
//! the page script ([`SCRIPT`], `flm.preview.script`) and its stylesheet ([`STYLE`],
//! `flm.preview.style`).
//!
//! The preview's builds require both definitions, together with the HTML backend's
//! stylesheet for the marks of errors, through the HTML backend's render option
//! [`required_definitions`](flm_core::stdlib::backends::html::HtmlRenderOptions::required_definitions)
//! ([`crate::watch::build_settings`]). A build that does not require them, such as one of
//! `flm render` or `flm check`, writes nothing of the preview into its page. The feature
//! is nevertheless registered under every command, because the registry is the same for
//! every command.
//!
//! The feature declares no settings and registers no stage. Like every feature of `std`,
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

/// The text of the page script: JavaScript that current browsers run as it is, one
/// function that defines no global name and changes no built-in object. Its first line is a
/// comment holding [`SCRIPT`].
///
/// The page script is part of the *watch client*, the npm package `flm-watchclient`,
/// whose sources are in the folder `npm/flm-watchclient/` of FLM's repository, written in
/// TypeScript. The package's build bundles the page script into one file, which is
/// committed in the folder `watch/client/` of this crate's sources (`flm-watchclient.js`);
/// this text is that file, after the line holding [`SCRIPT`]. The file holds the library
/// the script uses to update the page in place, `morphdom`; its first lines hold the
/// library's license. The script adds no visible element to the page: it marks the
/// elements the last build's problems lie in with an attribute, which [`STYLE_TEXT`]
/// outlines. The file `PROTOCOL.md` of the package specifies what the script expects of
/// the page and of the server it connects to, and the messages it exchanges with the
/// preview's tools page when the page is shown in that page's frame. The [`crate::watch`]
/// module's documentation and the command-line guide's section on the live preview
/// describe what the script does.
pub const SCRIPT_TEXT: &str = concat!("/* flm.preview.script */\n", include_str!("../watch/client/flm-watchclient.js"));

/// The text of the page script's stylesheet (the marks of problems, the flash of an
/// element shown, the outline of the element Inspect selected, and the server's waiting
/// page): the stylesheet of the npm package `flm-watchclient` ([`SCRIPT_TEXT`] says where
/// it comes from), the file `watch/client/flm-watchclient.css` of this crate's sources,
/// after a first line that is a comment holding [`STYLE`].
pub const STYLE_TEXT: &str = concat!("/* flm.preview.style */\n", include_str!("../watch/client/flm-watchclient.css"));

/// The feature `preview`, which declares the script and the stylesheet of the live
/// preview's page: see the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct PreviewFeature;

impl FlmFeature for PreviewFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "The live preview's page")
            .docs(
                "the script that keeps a page in step with its document while `flm watch` runs, and its \
                 stylesheet",
            )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.style_def(StyleDef {
            id: Name::from(STYLE),
            content: vec![DefContent::Inline(String::from(STYLE_TEXT))],
            docs: String::from("the stylesheet of the live preview's page script"),
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
        use flm_core::stdlib::backends::html::page::{ORIGINAL_HTML_ATTRIBUTE, REGION_UPDATED_EVENT};
        assert!(SCRIPT_TEXT.contains(&format!("\"{REGION_UPDATED_EVENT}\"")));
        assert!(SCRIPT_TEXT.contains(&format!("\"{ORIGINAL_HTML_ATTRIBUTE}\"")));
    }
}
