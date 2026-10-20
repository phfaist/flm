//! The guest's own types of `flm:lang/rewrites/userdefs` ([`Expansion`],
//! [`CompiledTemplate`], [`TemplateLanguage`], [`Definition`]), the handler
//! trait [`Expand`], and the record-and-lend method
//! `UserDefinition::element_def_with`, the twin of the prelude's fragment
//! `lang-rewrites-userdefs.js`: it records the hook under the element's name
//! and lends it, in every run of `register`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{arg, emit_from_value, emit_to_value, get, not_shape, object};
use crate::adapter::durable::{DurableKind, DurableToken};
use crate::adapter::handles::{Handle, HandleKind};
use crate::adapter::values::{durable_from_value, durable_to_value, handle_to_value, is_durable, JsonForm};
use crate::api::lang::rewrites::userdefs::{DefinitionKind, DefinitionModes, ExpandPolicy};
use crate::api::model::aspects::BlockRole;
use crate::guest::abi;
use crate::guest::lang::mint::{ArgDef, ElementDef};
use crate::guest::lang::rewrites::userdefs::{ExpandCx, Expander, PlaceholderBindings, Staged, UserDefinition, ValueBindings};
use crate::guest::registry;
use crate::guest::lang::BuiltLang;
use crate::guest::techy::node::Node;
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
    Emit(Vec<Staged>),
}

/// `"default"` or `{"emit": [staged, …]}`; the host's `expansion_from_value`
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
/// (a mode variant's template) or the durable token a compilation answered
/// (`compile_template_with`), written back in the form it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledTemplate {
    /// How it crosses.
    form: TemplateForm,
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
        let reply = abi::call_module("lang/rewrites/userdefs", FUNCTION, vec![arg(lang, FUNCTION)?])?;
        TemplateLanguage::from_value(&reply)
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

impl UserDefinition {
    /// Mirrors [`UserDefinition::element_def_with`](crate::api::lang::rewrites::userdefs::UserDefinition::element_def_with):
    /// the declaration of the definition's element, expanded by `hook`,
    /// which is recorded under `element_name` in the running `register`'s
    /// record and lent (in every run of `register`, a rehearsal's too).
    ///
    /// # Errors
    ///
    /// No `register` runs ([`ExtError::Handler`]), or the host's refusal.
    pub fn element_def_with(&self, owner: &str, element_name: &str, hook: Arc<dyn Expand>) -> Result<ElementDef, ExtError> {
        const METHOD: &str = "element_def_with";
        let lent = registry::lend_expand(element_name, &hook)?;
        let reply = abi::call(self.handle, METHOD, vec![arg(owner, METHOD)?, arg(element_name, METHOD)?, lent])?;
        JsonForm::from_value(&reply)
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
