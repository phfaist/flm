//! The configuration of the [`defterm`](super) feature, [`DeftermConfig`]:
//! the lead-in template that opens every term definition.

use std::hash::{Hash, Hasher};

/// The lead-in template of every term definition by default, `#{term}: `:
/// the term, a colon and a space, run into the first paragraph of the term
/// definition, as in `Entropy: A measure of disorder.`
pub const DEFAULT_LEAD_IN: &str = "#{term}: ";

/// The configuration of the [`Defterm`](super::Defterm) feature: the
/// **lead-in**, the text that a term definition (a `defterm` environment)
/// shows before the words of its first paragraph, usually the term being
/// defined.
///
/// The lead-in is a template in the [template
/// language](crate::guide::concepts_overview#the-template-language-shared-by-features)
/// (described for authors under [the template
/// language](crate::guide::language_syntax#the-template-language)), with one
/// placeholder: `#{term}` is the term as the term definition writes it,
/// markup included. The rest of the template is ordinary FLM markup, and
/// markup may enclose the placeholder. `#{term}: ` is the default, and
/// `\textbf{Definition} (\emph{#{term}}): ` a richer lead-in. A template
/// without `#{term}` is allowed: every term definition then opens with the
/// same words. A template may use `#{term}` once at most, because the term is
/// shown only once, where the term definition is written.
///
/// `None` sets no lead-in: a term definition is then its body alone. A blank
/// template is not a way to say so: it fails the registry build.
///
/// The value is what [`Defterm::new`](super::Defterm::new) takes, and a host
/// passes it through
/// [`StdcoreConfig::defterm`](crate::stdcore::StdcoreConfig::defterm). It is
/// the feature's configuration, fixed when the registry is built (see
/// [settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)):
/// the template is compiled once then, and a template that does not compile
/// fails the registry build with a message that quotes it. It enters the
/// registry digest ([`digest_bytes`](DeftermConfig::digest_bytes)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::defterm::{DeftermConfig, DEFAULT_LEAD_IN};
///
/// assert_eq!(DeftermConfig::new().lead_in.as_deref(), Some(DEFAULT_LEAD_IN));
///
/// let richer = DeftermConfig::new()
///     .lead_in(Some(r"\textbf{Definition} (\emph{#{term}}): ".to_string()));
/// assert!(richer.lead_in.is_some());
///
/// // No lead-in: a definition is its body alone.
/// assert!(DeftermConfig::new().lead_in(None).lead_in.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DeftermConfig {
    /// The lead-in template, or `None` for no lead-in. [`DEFAULT_LEAD_IN`]
    /// by default.
    pub lead_in: Option<String>,
}

impl Default for DeftermConfig {
    fn default() -> DeftermConfig {
        DeftermConfig { lead_in: Some(DEFAULT_LEAD_IN.to_string()) }
    }
}

impl DeftermConfig {
    /// The default configuration: the lead-in [`DEFAULT_LEAD_IN`].
    pub fn new() -> DeftermConfig {
        DeftermConfig::default()
    }

    /// Builder: sets the lead-in template, or `None` for no lead-in.
    pub fn lead_in(mut self, lead_in: Option<String>) -> DeftermConfig {
        self.lead_in = lead_in;
        self
    }

    /// The configuration written out as bytes: what the feature feeds into
    /// the registry digest, and what the `Hash` implementation hashes. `-`
    /// for no lead-in, else `+`, the template's length and the template.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match &self.lead_in {
            None => out.push(b'-'),
            Some(text) => {
                out.push(b'+');
                out.extend_from_slice(&(text.len() as u64).to_le_bytes());
                out.extend_from_slice(text.as_bytes());
            }
        }
        out
    }
}

impl Hash for DeftermConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_bytes_tell_every_lead_in_apart() {
        let default = DeftermConfig::new().digest_bytes();
        let richer =
            DeftermConfig::new().lead_in(Some("\\textbf{#{term}}: ".to_string())).digest_bytes();
        let none = DeftermConfig::new().lead_in(None).digest_bytes();
        let empty = DeftermConfig::new().lead_in(Some(String::new())).digest_bytes();
        assert_ne!(default, richer);
        assert_ne!(default, none);
        assert_ne!(none, empty);
    }
}
