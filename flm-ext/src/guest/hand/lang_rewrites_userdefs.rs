//! The guest's own types of `flm:lang/rewrites/userdefs` ([`Expansion`],
//! [`StagedNodes`], [`CompiledTemplate`], [`TemplateLanguage`],
//! [`Definition`], [`ExtraValues`]), the handler
//! trait [`Expand`], and the record-and-lend method
//! `UserDefinition::element_def_with`, the twin of the prelude's fragment
//! `lang-rewrites-userdefs.js`: it records the hook under the element's name
//! and lends it, in every run of `register`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{emit_from_value, emit_to_value, get, not_shape, object};
#[cfg(feature = "native")]
use crate::adapter::dispatch::{entry, extra, lend_durable_token};
#[cfg(feature = "native")]
use crate::adapter::durable::DurableTemplateFacade;
#[cfg(feature = "native")]
use crate::adapter::handles::{CallTable, Entry};
#[cfg(feature = "native")]
use crate::adapter::typed::{FromApi, IntoApi};
#[cfg(feature = "native")]
use crate::adapter::values::durable_table_for;
use crate::adapter::durable::{DurableKind, DurableToken};
use crate::adapter::handles::{Handle, HandleKind};
#[cfg(feature = "native")]
use crate::adapter::values::push_staged;
use crate::adapter::values::{durable_from_value, durable_to_value, handle_to_value, is_durable, JsonForm};
#[cfg(feature = "native")]
use crate::api;
#[cfg(feature = "native")]
use crate::binding::natives::{self, Act};
use crate::api::lang::rewrites::userdefs::{DefinitionKind, DefinitionModes, ExpandPolicy, PlaceholderRef};
use crate::api::model::aspects::BlockRole;
use crate::guest::abi;
#[cfg(not(feature = "native"))]
use crate::guest::abi::ToArg;
use crate::guest::hand::joined::sequence_type;
use crate::guest::lang::mint::{ArgDef, ElementDef};
use crate::guest::lang::rewrites::userdefs::{ExpandCx, Expander, PlaceholderBindings, Staged, UserDefinition, ValueBindings};
use crate::guest::registry;
use crate::guest::lang::BuiltLang;
use crate::guest::techy::node::{Node, Tree};
use crate::runtime::ExtError;

/// Mirrors [`Expand`](crate::api::lang::rewrites::userdefs::Expand): the
/// hook of an element that expands into other nodes.
pub trait Expand: Send + Sync {
    /// Mirrors [`Expand::policy`](crate::api::lang::rewrites::userdefs::Expand::policy):
    /// when the hook runs; by default once per plan.
    fn policy(&self) -> ExpandPolicy {
        ExpandPolicy::PerPlan
    }

    /// Mirrors [`Expand::expand`](crate::api::lang::rewrites::userdefs::Expand::expand):
    /// the expansion of `node`.
    ///
    /// # Errors
    ///
    /// What the hook answers.
    fn expand(&self, node: &Node, cx: &mut ExpandCx, xp: &mut Expander) -> Result<Expansion, ExtError>;
}

/// Mirrors [`Expansion`](crate::api::lang::rewrites::userdefs::Expansion):
/// what an expansion hook answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    /// The element's default expansion.
    Default,
    /// The node expands to these staged nodes.
    Emit(StagedNodes),
}

sequence_type! {
    /// Several staged nodes, in order: what a method answers in place of a
    /// list of staged nodes (what a lowerer, a restager or an expander
    /// staged), and what a function, a presentation node or an answer that
    /// takes several staged nodes is given.
    ///
    /// A sequence that the host answered is one handle and its length, and
    /// costs nothing until it is read: [`get`](StagedNodes::get) and
    /// [`iter`](StagedNodes::iter) fetch its staged nodes from the host, a
    /// crossing for an index and one for each group iterated. An extension
    /// builds its own sequence empty ([`Default`]) or from a `Vec` of staged
    /// nodes, and joins staged nodes and other sequences to it with
    /// [`push`](StagedNodes::push) and [`push_all`](StagedNodes::push_all),
    /// with no crossing: a sequence the host answered is passed back as its
    /// handle, whatever its length. [`to_vec`](StagedNodes::to_vec) fetches
    /// every staged node.
    StagedNodes of Staged, "staged node", api api::lang::rewrites::userdefs::Staged, entry StagedNodes, push push_staged
}

/// The sequence of the one staged node `staged`.
impl From<Staged> for StagedNodes {
    fn from(staged: Staged) -> StagedNodes {
        StagedNodes::from(vec![staged])
    }
}

/// `"default"` or `{"emit": staged}`; the host's `expansion_from_value`
/// and `expansion_to_value`.
impl JsonForm for Expansion {
    fn to_value(&self) -> Value {
        match self {
            Expansion::Default => Value::String("default".to_string()),
            Expansion::Emit(staged) => emit_to_value(staged),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        Ok(emit_from_value("an expansion", "default", value)?.map_or(Expansion::Default, Expansion::Emit))
    }
}

/// The api's `Expansion`, each staged node the entry of its handle; the
/// host's `expansion_from_value`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for Expansion {
    type Api = api::lang::rewrites::userdefs::Expansion;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        Ok(match self {
            Expansion::Default => api::lang::rewrites::userdefs::Expansion::Default,
            Expansion::Emit(staged) => api::lang::rewrites::userdefs::Expansion::Emit(staged.into_api(table)?),
        })
    }
}

/// The error of a durable token of another kind than `expected`.
fn durable_mismatch(expected: DurableKind, actual: DurableKind) -> ExtError {
    ExtError::Argument {
        method: String::new(),
        detail: format!("a durable token of the kind `{}` is expected here, not one of the kind `{}`", expected.name(), actual.name()),
    }
}

/// The durable token `value` writes, of the kind `kind`.
fn durable_of(value: &Value, kind: DurableKind) -> Result<DurableToken, ExtError> {
    let token = durable_from_value(value)?;
    if token.kind != kind {
        return Err(durable_mismatch(kind, token.kind));
    }
    Ok(token)
}

/// How a compiled template crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateForm {
    /// A handle, for one call.
    Lent(Handle),
    /// A durable token, valid for the life of the feature's durable table.
    Durable(DurableToken),
}

/// Mirrors [`CompiledTemplate`](crate::api::lang::rewrites::userdefs::CompiledTemplate):
/// a template compiled for expansion. In the guest, the handle a call lent
/// (a mode variant's template or default value) or the durable token a
/// compilation answered (`compile_template_with`), written back in the form
/// it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledTemplate {
    /// How it crosses.
    form: TemplateForm,
}

impl CompiledTemplate {
    /// Mirrors [`CompiledTemplate::placeholders`](crate::api::lang::rewrites::userdefs::CompiledTemplate::placeholders):
    /// every placeholder of the template, in the tree's storage order, each
    /// with the text of its substitution argument; asked of the host on the
    /// template's handle, or on its durable token, which the host resolves in
    /// the running call.
    ///
    /// # Errors
    ///
    /// What the host answers.
    pub fn placeholders(&self) -> Result<Vec<PlaceholderRef>, ExtError> {
        const METHOD: &str = "placeholders";
        #[cfg(not(feature = "native"))]
        {
            let reply = match self.form {
                TemplateForm::Lent(handle) => abi::call(handle, METHOD, Vec::new())?,
                TemplateForm::Durable(token) => abi::call_durable(token, METHOD, Vec::new())?,
            };
            JsonForm::from_value(&reply)
        }
        #[cfg(feature = "native")]
        {
            abi::typed(METHOD, |table| {
                let handle = self.lent(table)?;
                let this = &**entry!(table, handle, CompiledTemplate, CompiledTemplate);
                let answer = this.placeholders();
                FromApi::from_api(answer, table)
            })
        }
    }
}

/// A handle of the kind `compiled_template`, or a durable token of that kind.
impl JsonForm for CompiledTemplate {
    fn to_value(&self) -> Value {
        match self.form {
            TemplateForm::Lent(handle) => handle_to_value(handle),
            TemplateForm::Durable(token) => durable_to_value(token),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let form = if is_durable(value) {
            TemplateForm::Durable(durable_of(value, DurableKind::CompiledTemplate)?)
        } else {
            TemplateForm::Lent(abi::handle_of(value, HandleKind::CompiledTemplate)?)
        };
        Ok(CompiledTemplate { form })
    }
}

#[cfg(feature = "native")]
impl CompiledTemplate {
    /// The handle of the template in the running call: the one it was lent
    /// under, or, for a durable template, the handle the durable object its
    /// token names is lent under, as the dispatcher lends a durable token
    /// given as an argument (`resolve_durables`).
    ///
    /// # Errors
    ///
    /// The token names no object of the call's durable table
    /// ([`ExtError::StaleDurable`]), or the call holds none
    /// ([`ExtError::Argument`]).
    pub(crate) fn lent(&self, table: &mut CallTable<'_>) -> Result<Handle, ExtError> {
        match self.form {
            TemplateForm::Lent(handle) => Ok(handle),
            TemplateForm::Durable(token) => lend_durable_token(table, token),
        }
    }

    /// `f` given the api's template this one stands for: the entry of its
    /// handle, or the durable template its token names in the call's durable
    /// table, as the host's `extra_values_from_value` reads a template.
    ///
    /// # Errors
    ///
    /// A stale handle, or one of another kind; as [`lent`](CompiledTemplate::lent)
    /// for a token.
    fn with_api<R>(
        &self,
        table: &mut CallTable<'_>,
        f: impl FnOnce(&dyn api::lang::rewrites::userdefs::CompiledTemplate) -> R,
    ) -> Result<R, ExtError> {
        match self.form {
            TemplateForm::Lent(handle) => match table.entry(handle)? {
                Entry::CompiledTemplate(template) => Ok(f(&**template)),
                other => Err(crate::adapter::values::mismatch(HandleKind::CompiledTemplate, other.kind())),
            },
            TemplateForm::Durable(token) => Ok(f(&durable_table_for(table, token)?.compiled_template(token)?)),
        }
    }
}

/// A template the call lent (a mode variant's template or default value),
/// lent as its entry and read as its handle.
#[cfg(feature = "native")]
impl<'c, 'a: 'c> FromApi<'c, Box<dyn api::lang::rewrites::userdefs::CompiledTemplate + 'a>> for CompiledTemplate {
    fn from_api(api: Box<dyn api::lang::rewrites::userdefs::CompiledTemplate + 'a>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(CompiledTemplate { form: TemplateForm::Lent(table.lend(Entry::CompiledTemplate(api))) })
    }
}

/// A durable template (`compile_template_with`'s), read as its durable
/// token.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, DurableTemplateFacade> for CompiledTemplate {
    fn from_api(api: DurableTemplateFacade, _: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(CompiledTemplate { form: TemplateForm::Durable(api.token()) })
    }
}

/// Mirrors [`TemplateLanguage`](crate::api::lang::rewrites::userdefs::TemplateLanguage):
/// the language a feature's templates are compiled in, as its durable token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateLanguage {
    /// Its durable token.
    token: DurableToken,
}

impl TemplateLanguage {
    /// Mirrors [`TemplateLanguage::new`](crate::api::lang::rewrites::userdefs::TemplateLanguage::new):
    /// the template language of the built language `lang`.
    ///
    /// # Errors
    ///
    /// What the host answers.
    pub fn new(lang: &BuiltLang) -> Result<TemplateLanguage, ExtError> {
        const FUNCTION: &str = "TemplateLanguage::new";
        #[cfg(not(feature = "native"))]
        {
            let reply = abi::call_module("lang/rewrites/userdefs", FUNCTION, vec![ToArg::arg(lang)])?;
            TemplateLanguage::from_value(&reply)
        }
        #[cfg(feature = "native")]
        {
            abi::typed(FUNCTION, |table| {
                let language = extra::template_language_new(table, lang.handle)?;
                FromApi::from_api(language, table)
            })
        }
    }
}

/// A durable token of the kind `template_language`.
impl JsonForm for TemplateLanguage {
    fn to_value(&self) -> Value {
        durable_to_value(self.token)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        Ok(TemplateLanguage { token: durable_of(value, DurableKind::TemplateLanguage)? })
    }
}

/// The api's template language its token names, resolved against the call's
/// durable table, as the dispatcher lends a durable token given as an
/// argument (`lend_durable`).
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &TemplateLanguage {
    type Api = api::lang::rewrites::userdefs::TemplateLanguage;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        durable_table_for(table, self.token)?.template_language(self.token)?;
        Ok(api::lang::rewrites::userdefs::TemplateLanguage::from_token(self.token))
    }
}

/// The api's template language, read as its durable token.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::lang::rewrites::userdefs::TemplateLanguage> for TemplateLanguage {
    fn from_api(api: api::lang::rewrites::userdefs::TemplateLanguage, _: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(TemplateLanguage { token: api.token() })
    }
}

impl UserDefinition {
    /// Mirrors [`UserDefinition::element_def_with`](crate::api::lang::rewrites::userdefs::UserDefinition::element_def_with):
    /// the declaration of the definition's element, expanded by `hook`,
    /// which is recorded under `element_name` in the running `register`'s
    /// record and lent (in every run of `register`, a rehearsal's too).
    /// Compiled into a program, the declaration is the API's, lent into the
    /// running call.
    ///
    /// # Errors
    ///
    /// No `register` runs ([`ExtError::Handler`]), or the host's refusal.
    pub fn element_def_with(&self, owner: &str, element_name: &str, hook: Arc<dyn Expand>) -> Result<ElementDef, ExtError> {
        const METHOD: &str = "element_def_with";
        let policy = registry::record_expand(element_name, &hook)?;
        #[cfg(not(feature = "native"))]
        {
            let lent = registry::lend_expand(element_name, policy)?;
            let reply = abi::call(self.handle, METHOD, vec![ToArg::arg(owner), ToArg::arg(element_name), handle_to_value(lent)])?;
            JsonForm::from_value(&reply)
        }
        #[cfg(feature = "native")]
        {
            let handle = self.handle;
            abi::in_frame(Act::Expand(element_name), METHOD, |view| {
                let hook = natives::lend_expand(view, element_name.to_string(), policy);
                let this = entry!(view.table, handle, UserDefinition, UserDefinition).clone();
                FromApi::from_api(this.element_def_with(owner, element_name, hook), view.table)
            })
        }
    }
}

/// Mirrors the api's `impl PlaceholderBindings for ValueBindings`: value
/// bindings given where placeholder bindings are taken
/// (`Expander::instantiate`, `instantiate`), as their handle, which the
/// host takes for either kind. The value is for sending only: its handle is
/// of the kind `value_bindings`, so `PlaceholderBindings`' own reading
/// (`JsonForm::from_value`, which takes the kind `placeholder_bindings`)
/// refuses its form.
impl From<ValueBindings> for PlaceholderBindings {
    fn from(bindings: ValueBindings) -> PlaceholderBindings {
        PlaceholderBindings { handle: bindings.handle }
    }
}

/// Mirrors [`ExtraValues`](crate::api::lang::rewrites::userdefs::ExtraValues):
/// the values a hook computed for the extra placeholders of a template, each
/// under the placeholder's name and the text of its substitution argument
/// (`None` for a placeholder written without one); a pair given twice keeps
/// the later value. The guest's own: it holds the compiled templates (a
/// variant's template or default value, or a template that
/// `compile_template_with` answered) and the trees it is given, in the order
/// given, and the host builds the values from them when
/// [`over`](ExtraValues::over) is called.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtraValues {
    /// The values, in the order given: the name, the substitution text, the
    /// value.
    entries: Vec<(String, Option<String>, ExtraValue)>,
}

/// One value of [`ExtraValues`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExtraValue {
    /// A compiled template, instantiated in place.
    Template(CompiledTemplate),
    /// A tree, whose root's children are substituted.
    Tree(Tree),
    /// No value.
    NoValue,
}

impl ExtraValues {
    /// Mirrors [`ExtraValues::new`](crate::api::lang::rewrites::userdefs::ExtraValues::new):
    /// values that hold nothing.
    pub fn new() -> ExtraValues {
        ExtraValues::default()
    }

    /// Mirrors [`ExtraValues::template`](crate::api::lang::rewrites::userdefs::ExtraValues::template):
    /// gives the extra placeholder `name`, written with the substitution
    /// argument whose text is `substitution`, the compiled template
    /// `template`: a variant's template or default value, or a template that
    /// `compile_template_with` answered.
    pub fn template(self, name: &str, substitution: Option<&str>, template: CompiledTemplate) -> ExtraValues {
        self.push(name, substitution, ExtraValue::Template(template))
    }

    /// Mirrors [`ExtraValues::tree`](crate::api::lang::rewrites::userdefs::ExtraValues::tree):
    /// gives the extra placeholder `name`, written with the substitution
    /// argument whose text is `substitution`, the tree `tree`.
    pub fn tree(self, name: &str, substitution: Option<&str>, tree: &Tree) -> ExtraValues {
        self.push(name, substitution, ExtraValue::Tree(*tree))
    }

    /// Mirrors [`ExtraValues::no_value`](crate::api::lang::rewrites::userdefs::ExtraValues::no_value):
    /// gives the extra placeholder `name`, written with the substitution
    /// argument whose text is `substitution`, no value.
    pub fn no_value(self, name: &str, substitution: Option<&str>) -> ExtraValues {
        self.push(name, substitution, ExtraValue::NoValue)
    }

    /// Mirrors [`ExtraValues::over`](crate::api::lang::rewrites::userdefs::ExtraValues::over):
    /// the bindings that answer an extra placeholder from these values and
    /// every other placeholder from `base`.
    ///
    /// # Errors
    ///
    /// What the host answers.
    pub fn over(&self, base: &PlaceholderBindings) -> Result<PlaceholderBindings, ExtError> {
        const FUNCTION: &str = "ExtraValues::over";
        #[cfg(not(feature = "native"))]
        {
            let reply = abi::call_module("lang/rewrites/userdefs", FUNCTION, vec![ToArg::arg(self), ToArg::arg(base)])?;
            PlaceholderBindings::from_value(&reply)
        }
        #[cfg(feature = "native")]
        {
            abi::typed(FUNCTION, |table| {
                let values = IntoApi::into_api(self, table)?;
                let bindings = extra::extra_values_over(table, &values, base.handle)?;
                FromApi::from_api(bindings, table)
            })
        }
    }

    /// Appends `value` under `name` and `substitution`.
    fn push(mut self, name: &str, substitution: Option<&str>, value: ExtraValue) -> ExtraValues {
        self.entries.push((name.to_string(), substitution.map(str::to_string), value));
        self
    }
}

/// A list of `{"name", "substitution", "value"}`, the value `{"template":
/// template}`, `{"tree": tree}` or `"no-value"`; the host's
/// `extra_values_from_value`.
impl JsonForm for ExtraValues {
    fn to_value(&self) -> Value {
        let entries = self.entries.iter().map(|(name, substitution, value)| {
            let value = match value {
                ExtraValue::Template(template) => json!({ "template": template.to_value() }),
                ExtraValue::Tree(tree) => json!({ "tree": tree.to_value() }),
                ExtraValue::NoValue => Value::String("no-value".to_string()),
            };
            json!({ "name": name, "substitution": substitution, "value": value })
        });
        Value::Array(entries.collect())
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "the value of an extra placeholder";
        let Value::Array(items) = value else { return Err(not_shape("the values of extra placeholders", "a list", value)) };
        let mut values = ExtraValues::new();
        for item in items {
            let map = object(WHAT, item)?;
            let name: String = get(WHAT, map, "name")?;
            let substitution: Option<String> = get(WHAT, map, "substitution")?;
            let entry = match map.get("value") {
                Some(Value::String(text)) if text == "no-value" => ExtraValue::NoValue,
                Some(Value::Object(form)) if form.len() == 1 && form.contains_key("template") => {
                    ExtraValue::Template(CompiledTemplate::from_value(&form["template"])?)
                }
                Some(Value::Object(form)) if form.len() == 1 && form.contains_key("tree") => ExtraValue::Tree(Tree::from_value(&form["tree"])?),
                other => {
                    return Err(not_shape(
                        &format!("the field `value` of {WHAT}"),
                        "`{\"template\": template}`, `{\"tree\": tree}` or `\"no-value\"`",
                        other.unwrap_or(&Value::Null),
                    ))
                }
            };
            values = values.push(&name, substitution.as_deref(), entry);
        }
        Ok(values)
    }
}

/// The api's `ExtraValues`, given the values in order: each template the
/// entry of its handle or the durable template its token names, each tree
/// copied out of the table; the host's `extra_values_from_value`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &ExtraValues {
    type Api = api::lang::rewrites::userdefs::ExtraValues;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        let mut values = api::lang::rewrites::userdefs::ExtraValues::new();
        for (name, substitution, value) in &self.entries {
            let substitution = substitution.as_deref();
            values = match value {
                ExtraValue::Template(template) => template.with_api(table, |template| values.template(name, substitution, template))?,
                ExtraValue::Tree(tree) => {
                    let tree = tree.into_api(table)?;
                    values.tree(name, substitution, &*tree)
                }
                ExtraValue::NoValue => values.no_value(name, substitution),
            };
        }
        Ok(values)
    }
}

/// Mirrors `flm-core`'s `Definition`: a user definition's declaration. The
/// guest's own: its arguments are the guest's [`ArgDef`]s, one of which may
/// be a helper's (a handle).
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// The defined name.
    pub name: String,
    /// What is defined.
    pub kind: DefinitionKind,
    /// Its arguments.
    pub args: Vec<ArgDef>,
    /// Its replacement text, if any.
    pub content: Option<String>,
    /// The modes it is defined in.
    pub modes: DefinitionModes,
    /// The default of each optional argument, by name.
    pub defaults: BTreeMap<String, String>,
    /// The number of the first argument in the replacement text, less one.
    pub argument_number_offset: usize,
    /// Its block role.
    pub block_role: BlockRole,
    /// When it expands.
    pub expand: ExpandPolicy,
    /// Whether it overrides a definition of the same name.
    pub overrides: bool,
    /// Its documentation.
    pub docs: String,
    /// The extra placeholders, as `(name, substitution)`.
    pub extra_placeholders: Vec<(String, bool)>,
}

impl Definition {
    /// Mirrors `Definition::new`: a definition with the defaults of
    /// `flm-core`.
    pub fn new(kind: DefinitionKind, name: impl Into<String>, args: Vec<ArgDef>) -> Definition {
        Definition {
            name: name.into(),
            kind,
            args,
            content: None,
            modes: DefinitionModes::default(),
            defaults: BTreeMap::new(),
            argument_number_offset: 0,
            block_role: BlockRole::DEFAULT,
            expand: ExpandPolicy::default(),
            overrides: false,
            docs: String::new(),
            extra_placeholders: Vec::new(),
        }
    }

    /// Mirrors `Definition::macro_`: a macro definition; the trailing
    /// underscore avoids Rust's reserved word `macro`.
    pub fn macro_(name: impl Into<String>, args: Vec<ArgDef>) -> Definition {
        Definition::new(DefinitionKind::Macro, name, args)
    }

    /// Mirrors `Definition::environment`: an environment definition.
    pub fn environment(name: impl Into<String>, args: Vec<ArgDef>) -> Definition {
        Definition::new(DefinitionKind::Environment, name, args)
    }

    /// Mirrors `Definition::specials`: a special character sequence
    /// definition, whose use site is the characters `trigger`, written
    /// without a backslash, followed by the arguments.
    pub fn specials(trigger: impl Into<String>, args: Vec<ArgDef>) -> Definition {
        Definition::new(DefinitionKind::Specials, trigger, args)
    }

    /// Mirrors `Definition::text`: the replacement text, for text mode only
    /// ([`content`](Definition::content), then [`modes`](Definition::modes)
    /// with [`DefinitionModes::Text`]).
    pub fn text(self, template: impl Into<String>) -> Definition {
        self.content(template).modes(DefinitionModes::Text)
    }

    /// Mirrors `Definition::math`: the replacement text, for mathematics mode
    /// only ([`content`](Definition::content), then
    /// [`modes`](Definition::modes) with [`DefinitionModes::Math`]).
    pub fn math(self, template: impl Into<String>) -> Definition {
        self.content(template).modes(DefinitionModes::Math)
    }

    /// Mirrors `Definition::content`.
    pub fn content(mut self, text: impl Into<String>) -> Definition {
        self.content = Some(text.into());
        self
    }

    /// Mirrors `Definition::modes`.
    pub fn modes(mut self, modes: DefinitionModes) -> Definition {
        self.modes = modes;
        self
    }

    /// Mirrors `Definition::default`: the default of the optional argument
    /// `name`.
    pub fn default(mut self, name: impl Into<String>, template: impl Into<String>) -> Definition {
        self.defaults.insert(name.into(), template.into());
        self
    }

    /// Mirrors `Definition::argument_number_offset`.
    pub fn argument_number_offset(mut self, offset: usize) -> Definition {
        self.argument_number_offset = offset;
        self
    }

    /// Mirrors `Definition::block_role`.
    pub fn block_role(mut self, role: BlockRole) -> Definition {
        self.block_role = role;
        self
    }

    /// Mirrors `Definition::expand`.
    pub fn expand(mut self, policy: ExpandPolicy) -> Definition {
        self.expand = policy;
        self
    }

    /// Mirrors `Definition::overriding`.
    pub fn overriding(mut self) -> Definition {
        self.overrides = true;
        self
    }

    /// Mirrors `Definition::docs`.
    pub fn docs(mut self, text: impl Into<String>) -> Definition {
        self.docs = text.into();
        self
    }

    /// Mirrors `Definition::extra_placeholder`.
    pub fn extra_placeholder(mut self, name: impl Into<String>, substitution: bool) -> Definition {
        self.extra_placeholders.push((name.into(), substitution));
        self
    }
}

/// `{"kind", "name", "args", "content", "modes", "defaults",
/// "argument_number_offset", "block_role", "expand", "overrides", "docs",
/// "extra_placeholders": [{"name", "substitution"}]}`; the host's
/// `definition_from_value` and `definition_to_value`.
impl JsonForm for Definition {
    fn to_value(&self) -> Value {
        let extras: Vec<Value> = self.extra_placeholders.iter().map(|(name, substitution)| json!({ "name": name, "substitution": substitution })).collect();
        json!({
            "kind": self.kind.to_value(),
            "name": self.name,
            "args": self.args.iter().map(JsonForm::to_value).collect::<Vec<_>>(),
            "content": self.content,
            "modes": self.modes.to_value(),
            "defaults": self.defaults,
            "argument_number_offset": self.argument_number_offset,
            "block_role": self.block_role.to_value(),
            "expand": self.expand.to_value(),
            "overrides": self.overrides,
            "docs": self.docs,
            "extra_placeholders": extras,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a definition";
        let map = object(WHAT, value)?;
        let mut definition = Definition::new(get(WHAT, map, "kind")?, get::<String>(WHAT, map, "name")?, get::<Option<Vec<ArgDef>>>(WHAT, map, "args")?.unwrap_or_default());
        definition.content = get(WHAT, map, "content")?;
        if let Some(modes) = get(WHAT, map, "modes")? {
            definition.modes = modes;
        }
        match map.get("defaults") {
            None | Some(Value::Null) => {}
            Some(Value::Object(defaults)) => {
                for (key, template) in defaults {
                    let template = template.as_str().ok_or_else(|| not_shape(&format!("the default `{key}` of {WHAT}"), "a string", template))?;
                    definition.defaults.insert(key.clone(), template.to_string());
                }
            }
            Some(other) => return Err(not_shape(&format!("the field `defaults` of {WHAT}"), "a map", other)),
        }
        definition.argument_number_offset = get::<Option<usize>>(WHAT, map, "argument_number_offset")?.unwrap_or(0);
        if let Some(role) = get(WHAT, map, "block_role")? {
            definition.block_role = role;
        }
        if let Some(policy) = get(WHAT, map, "expand")? {
            definition.expand = policy;
        }
        definition.overrides = get::<Option<bool>>(WHAT, map, "overrides")?.unwrap_or(false);
        definition.docs = get::<Option<String>>(WHAT, map, "docs")?.unwrap_or_default();
        for extra in get::<Option<Vec<Value>>>(WHAT, map, "extra_placeholders")?.unwrap_or_default() {
            const EXTRA: &str = "an extra placeholder";
            let extra = object(EXTRA, &extra)?;
            definition.extra_placeholders.push((get(EXTRA, extra, "name")?, get::<Option<bool>>(EXTRA, extra, "substitution")?.unwrap_or(false)));
        }
        Ok(definition)
    }
}

/// The api's `Definition`, its arguments converted as the host reads an
/// argument list, its fields given as the host's `definition_from_value`
/// gives them.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &Definition {
    type Api = api::lang::rewrites::userdefs::Definition;

    fn into_api(self, _: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        let args = crate::guest::hand::lang_mint::args_to_core(&self.args);
        let mut definition = api::lang::rewrites::userdefs::Definition::new(self.kind, self.name.clone(), args);
        if let Some(text) = &self.content {
            definition = definition.content(text.clone());
        }
        definition = definition.modes(self.modes);
        for (key, template) in &self.defaults {
            definition = definition.default(key.clone(), template.as_str());
        }
        definition = definition.argument_number_offset(self.argument_number_offset).block_role(self.block_role).expand(self.expand);
        if self.overrides {
            definition = definition.overriding();
        }
        if !self.docs.is_empty() {
            definition = definition.docs(self.docs.clone());
        }
        for (name, substitution) in &self.extra_placeholders {
            definition = definition.extra_placeholder(name.clone(), *substitution);
        }
        Ok(definition)
    }
}

/// The api's `Definition` copied as the guest's, each argument as the guest
/// reads the host's `arg_def_to_value`; the host's `definition_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, &api::lang::rewrites::userdefs::Definition> for Definition {
    fn from_api(api: &api::lang::rewrites::userdefs::Definition, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(Definition {
            name: api.name.to_string(),
            kind: api.kind,
            args: FromApi::from_api(api.args.clone(), table)?,
            content: api.content.clone(),
            modes: api.modes,
            defaults: api.defaults.clone(),
            argument_number_offset: api.argument_number_offset,
            block_role: api.block_role,
            expand: api.expand,
            overrides: api.overrides,
            docs: api.docs.clone(),
            extra_placeholders: api.extra_placeholders.iter().map(|extra| (extra.name.clone(), extra.substitution)).collect(),
        })
    }
}
