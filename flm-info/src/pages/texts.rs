//! The documentation texts a page embeds as FLM, and whose text each is ([`texts`]).

use flm_core::core::registry::HOST_FEATURE;
#[cfg(feature = "flm-main")]
use serde_json::Value;

use super::Page;
use super::paths::LANGUAGE_OWNER;
use crate::env::Environment;
#[cfg(feature = "flm-main")]
use crate::host::OriginView;
#[cfg(feature = "flm-main")]
use crate::summary_of;

/// A documentation text that a page embeds as FLM, as [`texts`] lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedText {
    /// The text as the page embeds it: a documentation text, or the summary of one (its
    /// first paragraph), in FLM with the backtick of the extension `flmdoc`.
    pub text: String,
    /// Whose text it is.
    pub source: TextSource,
}

/// Whose text a documentation text is: who declared it, or where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextSource {
    /// A text that a feature of the registry declares, for itself or for what it declares
    /// (a command, an argument, a setting, a workflow, a name of a namespace).
    Feature {
        /// The feature.
        feature: String,
        /// The package that holds the feature, as the page `packages/` names it
        /// (`<identity>#<package>` for a package of an extension); `None` when no package
        /// of the environment holds it.
        package: Option<String>,
    },
    /// A text written as data in a settings value, by a package of an extension, in a
    /// document's front matter, or on the command line (cargo feature `flm-main`): the
    /// documentation text of a workflow given as data, or a text in the value of a feature's
    /// settings key, such as that of a user definition, when that key or entry alone holds it
    /// (see [`texts`]).
    #[cfg(feature = "flm-main")]
    Written {
        /// What holds the text.
        holder: TextHolder,
        /// Where it was written, as the settings cascade gives it.
        origin: Box<OriginView>,
    },
    /// A text that no feature declares and that no settings source wrote: the language's
    /// own (the owner `flm`), the host library's (the keys of a front matter), the
    /// program's (a workflow given to the registry on behalf of no feature, the owner
    /// `_host`, when where it was written is not known), or the pages' own (the summary of
    /// an index page).
    Flm,
}

/// The settings value that holds a documentation text written as data
/// ([`TextSource::Written`]).
#[cfg(feature = "flm-main")]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextHolder {
    /// The field `docs` of the workflow called `name`.
    Workflow {
        /// The workflow's name.
        name: String,
    },
    /// The value of the settings key `key` of the feature `feature`, or its entry `entry`
    /// for a key combined entry by entry (the macros of the user definitions, for example).
    Setting {
        /// The feature.
        feature: String,
        /// The key.
        key: String,
        /// The entry, for a key combined entry by entry.
        entry: Option<String>,
    },
}

/// Returns the documentation texts that `page` embeds as FLM, whole or as a summary, each
/// with whose text it is, in the order the page writes them; each pair of a text and a
/// source comes once.
///
/// The list is made by the walk that writes the page's source ([`source`](super::source)):
/// every documentation text the source embeds goes through one place, which records it with
/// its source, so the list holds exactly the texts that a program must be able to parse as
/// FLM for the page to render. A text written whole is listed with its summary when the two
/// differ, since the page writes the summary on its own (in `\docsummary`). A program that
/// renders the pages parses each text with the registry that renders them, and lists the
/// texts that do not parse in [`SourceOptions::plain_texts`](super::SourceOptions::plain_texts).
///
/// A text's source is the feature that declares what it documents, with that feature's
/// package. A text written as data comes from where it was written, as the settings cascade
/// gives it (cargo feature `flm-main`):
///
/// - the documentation text of a workflow whose field `docs` a settings source wrote, found
///   by the workflow's name;
/// - a text that a feature declares from its settings (a user definition's, for example),
///   when exactly one key of the feature's settings, or one entry of a key combined entry by
///   entry, holds it whole or as its first paragraph under a key `docs`, and a settings
///   source wrote that key or entry: the key's origin, or the entry's. The registry does not
///   record from which entry a declaration was made, so the text is found by comparison;
///   when several keys or entries hold it (two user definitions with the same text, or one
///   whose text is the first paragraph of another's), or when the one that holds it is an
///   entry of the feature's own, the text has the source of any other text a feature
///   declares: the feature and its package.
///
/// A text that no feature declares has the source [`TextSource::Flm`].
pub fn texts(page: &Page) -> Vec<EmbeddedText> {
    super::source::embedded(page)
}

/// The source of `text`, a documentation text declared by `owner` for itself or for what it
/// declares, in `env`: [`TextSource::Flm`] for no owner, the language's own name or the
/// host's ([`HOST_FEATURE`]); where it was written, for a text written as data in exactly one
/// of the owner's settings keys or entries (cargo feature `flm-main`); else the feature and
/// its package.
pub(super) fn declared(env: &Environment, owner: Option<&str>, text: &str) -> TextSource {
    let owner = match owner {
        None | Some(LANGUAGE_OWNER) | Some(HOST_FEATURE) => return TextSource::Flm,
        Some(owner) => owner,
    };
    #[cfg(feature = "flm-main")]
    if let Some(source) = written_in_settings(env, owner, text) {
        return source;
    }
    #[cfg(not(feature = "flm-main"))]
    let _ = text;
    TextSource::Feature { feature: owner.to_string(), package: env.package_of(owner) }
}

/// The source of `text`, the documentation text of the workflow called `name` (or its
/// summary), in `env`: where the workflow's field `docs` was written when a settings source
/// wrote it (cargo feature `flm-main`), else the feature that declares the workflow.
pub(super) fn workflow(env: &Environment, name: &str, text: &str) -> TextSource {
    #[cfg(feature = "flm-main")]
    if let Some(origin) = workflow_docs_origin(env, name) {
        let holder = TextHolder::Workflow { name: name.to_string() };
        return TextSource::Written { holder, origin: Box::new(origin.clone()) };
    }
    let owner = env.registry().workflow(name).map(|workflow| workflow.owner.to_string());
    declared(env, owner.as_deref(), text)
}

/// Where the field `docs` of the workflow called `name` was written, when a settings source
/// wrote it.
#[cfg(feature = "flm-main")]
fn workflow_docs_origin<'a>(env: &'a Environment, name: &str) -> Option<&'a OriginView> {
    env.host()?.workflows.get(name)?.get(DOCS_KEY)
}

/// The key under which a value written as data holds a documentation text: the field of a
/// workflow, of a user definition and of its arguments, of a heading kind.
#[cfg(feature = "flm-main")]
const DOCS_KEY: &str = "docs";

/// The source of `text` when it was written in the settings of `owner`: when exactly one key
/// or entry of those settings holds it (whole or as its summary) under a key `docs`, at any
/// depth, and a settings source wrote that key or entry, the key (or, for a key combined
/// entry by entry, the entry) with its origin.
///
/// Every key that a source wrote counts, and every entry of a key combined entry by entry,
/// the feature's own entries included (its default entries that no source replaced, in the
/// keys that a source wrote and in those that none wrote); the default value of any other
/// key, which the feature's settings declaration does not hold as data, is not compared. The
/// result is `None` when no key or entry holds the text, when several do, or when the one
/// that holds it is one of the feature's own: the registry does not record from which entry a
/// declaration was made, so a text with more than one possible place is credited to the
/// feature, never to a guess.
#[cfg(feature = "flm-main")]
fn written_in_settings(env: &Environment, owner: &str, text: &str) -> Option<TextSource> {
    let in_effect = env.host()?.settings.get(owner)?;
    let decl = env.factories().iter().find_map(|factory| factory.settings(owner));
    // Each key or entry that holds the text: the key, the entry, and where a source wrote it
    // (`None` for an entry of the feature's own).
    let mut found: Vec<(&str, Option<&str>, Option<&OriginView>)> = Vec::new();
    for (key, value) in &in_effect.keys {
        if decl.is_some_and(|decl| decl.default_entries(key).is_some()) {
            let Value::Object(entries) = &value.value else { continue };
            for (entry, entry_value) in entries {
                if holds(entry_value, text) {
                    found.push((key.as_str(), Some(entry.as_str()), value.entry_origins.get(entry)));
                }
            }
        } else if holds(&value.value, text) {
            found.push((key.as_str(), None, Some(&value.origin)));
        }
    }
    if let Some(decl) = decl {
        for mark in decl.marks().iter().filter(|mark| mark.merge_entries && !in_effect.keys.contains_key(&mark.path)) {
            for (entry, entry_value) in decl.default_entries(&mark.path).into_iter().flatten() {
                if holds(entry_value, text) {
                    found.push((mark.path.as_str(), Some(entry.as_str()), None));
                }
            }
        }
    }
    let [(key, entry, Some(origin))] = found.as_slice() else { return None };
    Some(TextSource::Written {
        holder: TextHolder::Setting { feature: owner.to_string(), key: key.to_string(), entry: entry.map(str::to_string) },
        origin: Box::new(OriginView::clone(origin)),
    })
}

/// Whether `value` holds `text`, or a text whose summary is `text`, under a key `docs` at
/// any depth.
#[cfg(feature = "flm-main")]
fn holds(value: &Value, text: &str) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, entry)| match entry {
            Value::String(written) if key == DOCS_KEY => written == text || summary_of(written) == text,
            _ => holds(entry, text),
        }),
        Value::Array(items) => items.iter().any(|item| holds(item, text)),
        _ => false,
    }
}
