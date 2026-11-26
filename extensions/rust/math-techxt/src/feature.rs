//! The registration of the feature `math-techxt`: the vocabulary, the
//! converter, the check and render stages, the element and the rules.

use flm_ext::ExtError;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::stdlib::backends::html::TAG;
use serde_json::Value;

use crate::math_techxt::check::register_check;
use crate::math_techxt::render::{self, register_render};
use crate::math_techxt::rules::{self, declare_peer_tables, register_command_rules, register_formula_rules};
use crate::math_techxt::settings::{self, TechxtSettings};
use crate::math_techxt::vocabulary::{Vocabulary, definitions};
use crate::math_techxt::RENDERED_AS_TEXT;

/// Registers the feature `math-techxt` with the configuration `settings`: the
/// commands a formula may use, declared from the converter's definitions; the
/// converter, built once ([`render::converter`]); the check of the formulas,
/// which runs on every workflow that validates a document, and its capability
/// ([`register_check`]); the stage that renders the formulas with the
/// converter, and its table, which the text and HTML rules read
/// ([`register_render`]); the element [`RENDERED_AS_TEXT`]; the rules, the
/// library's formula rules on the LaTeX outputs among them
/// ([`rules::latex`]), and the HTML stylesheet, loading the font from the
/// setting `font_url`; and the settings as an input of the registry digest,
/// since the address of the font changes the HTML output and no declaration
/// the registry digests holds it.
///
/// # Errors
///
/// The settings cannot be read ([`TechxtSettings::from_settings`]), worded as
/// the host reports the refusal of the feature's configuration reader
/// ([`settings::read`]); techxt refuses the converter's definitions; any
/// refusal of the host. A command that another feature declares in math mode
/// makes the registry build fail with the registry's own error for two
/// declarations of one command.
pub fn register(b: &mut FeatureBuilder, settings: &Value) -> Result<(), ExtError> {
    let settings = TechxtSettings::from_settings(settings).map_err(settings::refused)?;
    let owner = b.feature()?;
    let set = definitions();
    Vocabulary::of(&set, &techxt::defs::standard()).declare(b, &owner)?;
    let converter = render::converter(set)?;
    register_check(b)?;
    register_render(b, &converter)?;
    b.presentation_element(
        RENDERED_AS_TEXT,
        "A formula rendered as Unicode text by techxt: its rendered lines in the payload, and each text \
         command's content, lowered on the backend at hand, in a content slot of its own.",
    )?;
    b.peer_backend(rules::text::BACKEND, "the text backend, on which the extension's rules write the formulas as text")?;
    b.peer_backend(rules::html::BACKEND, "the HTML backend, on which the extension's rules write the formulas as text")?;
    b.peer_element(TAG, "the HTML backend's construct, which the HTML rules stage")?;
    declare_peer_tables(b)?;
    register_command_rules(b)?;
    for backend in [rules::text::BACKEND, rules::html::BACKEND] {
        register_formula_rules(b, backend)?;
    }
    rules::latex::register_formula_rules(b)?;
    rules::text::register_element_rule(b)?;
    rules::html::register_element_rule(b, &settings.font_url)?;
    b.digest_input(settings.font_url.as_bytes())
}
