//! Durable handles: objects the adapter keeps beyond the call that made them,
//! named by a token a handler may keep as plain data.
//!
//! Every handle of [`handles`](crate::adapter::handles) dies with its call. A
//! template language and a compiled template, though, are owned values that
//! borrow nothing (`flm-core`'s [`TemplateLanguage`] and
//! [`CompiledTemplate`]): a feature compiles a template once and instantiates
//! it at every use, on whichever thread runs the use. The adapter keeps such
//! objects in a [`DurableTable`], one per feature per registration, and a
//! handler names one by its [`DurableToken`] `{ kind, table, index }`, valid on
//! every thread and in every later call for the life of the table.
//!
//! **What makes a token stale.** A token is resolved against the table of the
//! running call's feature ([`DurableTable::get`]). A token of another table (a
//! table of another feature, or of another registry: a re-preparation builds
//! new tables), an index the table does not hold, or a kind that is not the
//! entry's, is [`ExtError::StaleDurable`]: never another object.
//!
//! **The digest.** Nothing durable enters the registry digest: an entry is
//! computed from the built language and the handler's specification, both
//! already digested (the language through the declarations, the
//! specification through the script's text and its configuration).
//!
//! The table is filled through [`DurableTable::language`] and
//! [`DurableTable::template`]; the façades that lend the table to a call
//! implement the crate's sealed trait `DurableOwner`.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use flm_core::lang::rewrites::userdefs::{
    compile_template_with, CompiledTemplate as CoreCompiledTemplate, PlaceholderVocabulary, TemplateLanguage,
    TemplateSpec,
};
use flm_core::lang::BuiltLang;
use serde_json::Value;
use flm_core::techy::latexlike::Mode;

use crate::api::lang::rewrites::userdefs::CompiledTemplate;
use crate::runtime::ExtError;
use crate::sealed::{CompiledTemplateCore, Token};

/// The kinds of object a [`DurableToken`] names. Its serde form is the
/// kind's name ([`name`](DurableKind::name)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableKind {
    /// A [`TemplateLanguage`]: the feature's one template language.
    TemplateLanguage,
    /// A [`CompiledTemplate`](CoreCompiledTemplate).
    CompiledTemplate,
}

impl DurableKind {
    /// The kind's name, as its serde form writes it (`template_language`,
    /// `compiled_template`).
    pub fn name(self) -> &'static str {
        match self {
            DurableKind::TemplateLanguage => "template_language",
            DurableKind::CompiledTemplate => "compiled_template",
        }
    }
}

/// The name of an entry of a [`DurableTable`]: what the entry is (`kind`),
/// which table holds it (`table`, the table's [`number`](DurableTable::number)),
/// and where (`index`). Plain data: a handler may keep it anywhere, and it is
/// valid on every thread for the life of the table. Across a JSON binding it is
/// written `{"@durable": {"kind", "table", "index"}}`.
///
/// A token is stale when it is resolved under a table other than its own (a
/// handler of another feature, or of the same feature under another registry:
/// each registration makes a new table), or when its table holds no entry of
/// its kind at its index; resolving a stale token is
/// [`ExtError::StaleDurable`], never another object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DurableToken {
    /// What the entry is.
    pub kind: DurableKind,
    /// The number of the table that holds it.
    pub table: u64,
    /// Its position in that table.
    pub index: u32,
}

/// An entry of a [`DurableTable`]: the object, shared.
#[derive(Debug, Clone)]
pub enum DurableEntry {
    /// The feature's template language.
    TemplateLanguage(Arc<TemplateLanguage>),
    /// A compiled template.
    CompiledTemplate(Arc<CoreCompiledTemplate>),
}

impl DurableEntry {
    /// The entry's kind.
    pub fn kind(&self) -> DurableKind {
        match self {
            DurableEntry::TemplateLanguage(_) => DurableKind::TemplateLanguage,
            DurableEntry::CompiledTemplate(_) => DurableKind::CompiledTemplate,
        }
    }
}

/// The key of a compiled template in a [`DurableTable`]: the data of its
/// [`TemplateSpec`], owned. The table's template language is the table's
/// one, so two compilations with equal keys give the same template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateKey {
    text: String,
    mode: Mode,
    vocabulary: PlaceholderVocabulary,
    offset: usize,
    definition: String,
    label: String,
    origin_label: String,
    user_definition: bool,
}

impl TemplateKey {
    /// The key of `spec`.
    pub fn of(spec: &TemplateSpec<'_>) -> TemplateKey {
        TemplateKey {
            text: spec.text.to_string(),
            mode: spec.mode,
            vocabulary: spec.vocabulary.clone(),
            offset: spec.offset,
            definition: spec.definition.to_string(),
            label: spec.label.to_string(),
            origin_label: spec.origin_label.to_string(),
            user_definition: spec.user_definition,
        }
    }
}

impl Hash for TemplateKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // `PlaceholderVocabulary` is not `Hash`: its fields are hashed here.
        self.text.hash(state);
        self.mode.hash(state);
        self.vocabulary.arity.hash(state);
        self.vocabulary.names.hash(state);
        self.vocabulary.body.hash(state);
        for extra in &self.vocabulary.extras {
            extra.name.hash(state);
            extra.substitution.hash(state);
        }
        self.offset.hash(state);
        self.definition.hash(state);
        self.label.hash(state);
        self.origin_label.hash(state);
        self.user_definition.hash(state);
    }
}

/// The source of the tables' numbers: process-wide, so that a token names one
/// table of one registry.
static NEXT_TABLE: AtomicU64 = AtomicU64::new(1);

/// What the table's mutex guards.
#[derive(Debug, Default)]
struct Entries {
    /// The entries, by index; none is ever removed.
    list: Vec<DurableEntry>,
    /// The index of the template language, once built.
    language: Option<u32>,
    /// The index of each compiled template, by its key.
    templates: HashMap<TemplateKey, u32>,
}

/// The durable objects of one feature of one registration: a list of shared
/// entries behind a mutex, its number, and the feature it belongs to (named in
/// the stale token's error).
///
/// The table lives as long as the handler objects of its registration hold
/// it, and dies with the registry; no entry is ever removed. Nothing durable
/// enters the registry digest (the [module documentation](self) says why).
#[derive(Debug)]
pub struct DurableTable {
    number: u64,
    extension: String,
    package: String,
    feature: String,
    entries: Mutex<Entries>,
}

impl DurableTable {
    /// A new, empty table for the feature `feature` of the package `package`
    /// of the extension whose stem is `extension`, with a number no other
    /// table of the process has.
    pub fn new(extension: &str, package: &str, feature: &str) -> DurableTable {
        DurableTable {
            number: NEXT_TABLE.fetch_add(1, Ordering::Relaxed),
            extension: extension.to_string(),
            package: package.to_string(),
            feature: feature.to_string(),
            entries: Mutex::new(Entries::default()),
        }
    }

    /// The table's number, which its tokens carry.
    pub fn number(&self) -> u64 {
        self.number
    }

    /// The feature's template language: built from `lang` at the first
    /// call, answered again at every later call (one per table; the language
    /// is the registry's, so one per feature per registry suffices). The
    /// mutex is held while the language is built, so that two threads build
    /// it once.
    ///
    /// # Errors
    ///
    /// The template language does not build: [`ExtError::Core`] of kind
    /// `template_language`, its detail the build errors' texts joined by `; `
    /// and its data the list of those texts. Nothing is kept then, and a
    /// later call builds again.
    pub fn language(&self, lang: &BuiltLang) -> Result<(DurableToken, Arc<TemplateLanguage>), ExtError> {
        let mut entries = self.lock();
        if let Some(index) = entries.language
            && let Some(DurableEntry::TemplateLanguage(language)) = entries.list.get(index as usize)
        {
            return Ok((self.token(DurableKind::TemplateLanguage, index), Arc::clone(language)));
        }
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            failure("template_language", errors.iter().map(ToString::to_string).collect())
        })?;
        let language = Arc::new(language);
        let index = push(&mut entries, DurableEntry::TemplateLanguage(Arc::clone(&language)));
        entries.language = Some(index);
        Ok((self.token(DurableKind::TemplateLanguage, index), language))
    }

    /// The template `spec` compiled against `lang` with the table's own
    /// template language ([`language`](DurableTable::language), built first
    /// if need be): compiled at the first call with that specification,
    /// answered again at every later call with an equal one, from any
    /// thread (the dedup key is the specification's data, [`TemplateKey`];
    /// the language is the table's one, so equal keys give equal templates).
    /// The mutex is held while the template compiles, so that two threads
    /// asking for one specification compile it once and get one index.
    ///
    /// # Errors
    ///
    /// As [`language`](DurableTable::language); the template does not
    /// compile: [`ExtError::Core`] of kind `compile_template`, its detail
    /// the compilation's diagnostics' texts joined by `; ` and its data the
    /// list of those texts. Nothing is kept then, and a later call compiles
    /// again.
    pub fn template(
        &self,
        spec: &TemplateSpec<'_>,
        lang: &BuiltLang,
    ) -> Result<(DurableToken, Arc<CoreCompiledTemplate>), ExtError> {
        let (_, language) = self.language(lang)?;
        let key = TemplateKey::of(spec);
        let mut entries = self.lock();
        if let Some(&index) = entries.templates.get(&key)
            && let Some(DurableEntry::CompiledTemplate(template)) = entries.list.get(index as usize)
        {
            return Ok((self.token(DurableKind::CompiledTemplate, index), Arc::clone(template)));
        }
        let template = compile_template_with(spec, lang, &language).map_err(|diagnostics| {
            failure("compile_template", diagnostics.iter().map(|diagnostic| diagnostic.message()).collect())
        })?;
        let template = Arc::new(template);
        let index = push(&mut entries, DurableEntry::CompiledTemplate(Arc::clone(&template)));
        entries.templates.insert(key, index);
        Ok((self.token(DurableKind::CompiledTemplate, index), template))
    }

    /// The number of entries the table holds.
    pub fn len(&self) -> usize {
        self.lock().list.len()
    }

    /// Whether the table holds no entry.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entry `token` names.
    ///
    /// # Errors
    ///
    /// [`ExtError::StaleDurable`] when the token is of another table, names an
    /// index the table does not hold, or names an entry of another kind.
    pub fn get(&self, token: DurableToken) -> Result<DurableEntry, ExtError> {
        let entries = self.lock();
        let entry = (token.table == self.number)
            .then(|| entries.list.get(token.index as usize))
            .flatten()
            .filter(|entry| entry.kind() == token.kind);
        entry.cloned().ok_or_else(|| self.stale(token))
    }

    /// The compiled template `token` names, as the extension API's façade,
    /// which owns the template's `Arc` and borrows nothing of the table.
    ///
    /// # Errors
    ///
    /// As [`get`](DurableTable::get).
    pub fn compiled_template(&self, token: DurableToken) -> Result<DurableTemplateFacade, ExtError> {
        match self.get(token)? {
            DurableEntry::CompiledTemplate(template) => Ok(DurableTemplateFacade { template, token }),
            DurableEntry::TemplateLanguage(_) => Err(self.stale(token)),
        }
    }

    /// The template language `token` names, with its token.
    ///
    /// # Errors
    ///
    /// As [`get`](DurableTable::get).
    pub fn template_language(&self, token: DurableToken) -> Result<Arc<TemplateLanguage>, ExtError> {
        match self.get(token)? {
            DurableEntry::TemplateLanguage(language) => Ok(language),
            DurableEntry::CompiledTemplate(_) => Err(self.stale(token)),
        }
    }

    /// The error of a token this table does not hold.
    fn stale(&self, token: DurableToken) -> ExtError {
        ExtError::StaleDurable {
            kind: token.kind,
            table: token.table,
            index: token.index,
            feature: self.feature.clone(),
            package: self.package.clone(),
            extension: self.extension.clone(),
        }
    }

    /// The token of the entry `index` of this table.
    fn token(&self, kind: DurableKind, index: u32) -> DurableToken {
        DurableToken { kind, table: self.number, index }
    }

    /// The entries. A thread that panicked while holding the lock left them
    /// whole (an entry is pushed after it is built), so a poisoned lock is
    /// taken as it is.
    fn lock(&self) -> MutexGuard<'_, Entries> {
        self.entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The failure of the `flm-core` function `kind` names, with `texts`.
fn failure(kind: &str, texts: Vec<String>) -> ExtError {
    ExtError::Core {
        kind: kind.to_string(),
        detail: texts.join("; "),
        data: Value::Array(texts.into_iter().map(Value::String).collect()),
    }
}

/// Pushes `entry` and answers its index.
fn push(entries: &mut Entries, entry: DurableEntry) -> u32 {
    // A table holds far fewer than `u32::MAX` entries: one per distinct
    // template a feature compiles.
    let index = u32::try_from(entries.list.len()).unwrap_or(u32::MAX);
    entries.list.push(entry);
    index
}

/// The façade of a durable compiled template: the template's `Arc` and the
/// token that names it. It borrows nothing, so it may outlive the call that
/// made it; [`Expander::instantiate`](crate::api::lang::rewrites::userdefs::Expander::instantiate)
/// takes it as it takes a definition's template.
pub struct DurableTemplateFacade {
    template: Arc<CoreCompiledTemplate>,
    token: DurableToken,
}

impl DurableTemplateFacade {
    /// The token that names the template.
    pub fn token(&self) -> DurableToken {
        self.token
    }
}

impl CompiledTemplateCore for DurableTemplateFacade {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&CoreCompiledTemplate)) {
        f(&self.template);
    }
}

impl CompiledTemplate for DurableTemplateFacade {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_s_serde_form_names_its_kind() {
        let token = DurableToken { kind: DurableKind::CompiledTemplate, table: 3, index: 1 };
        assert_eq!(
            serde_json::to_value(token).expect("a value"),
            serde_json::json!({ "kind": "compiled_template", "table": 3, "index": 1 })
        );
        assert_eq!(DurableKind::TemplateLanguage.name(), "template_language");
    }

    #[test]
    fn two_tables_have_two_numbers() {
        assert_ne!(DurableTable::new("wc", "_", "f").number(), DurableTable::new("wc", "_", "f").number());
    }

    #[test]
    fn an_empty_table_holds_no_token() {
        let table = DurableTable::new("wc", "core", "count");
        let token = DurableToken { kind: DurableKind::CompiledTemplate, table: table.number(), index: 0 };
        let error = table.get(token).expect_err("stale");
        assert_eq!(
            error.to_string(),
            "the `compiled_template` object 0 must be one the feature `count` of the package `core` of the \
             extension `wc` made under the registry in use"
        );
    }
}
