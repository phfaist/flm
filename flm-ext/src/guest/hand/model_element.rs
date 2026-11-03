//! The guest's own types of `flm:model/element` that hold handler objects:
//! [`ElementSpec`] and [`Aspect`], with the record-and-lend method
//! [`ElementSpec::with`], the twin of the prelude's fragment
//! `model-element.js`: it records the aspect's handler object under the
//! element and lends it, in every run of `register` (a rehearsal's too), and
//! keeps the aspect's crossing form ([`AspectForm`]).

use std::fmt;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{get, not_shape, object};
#[cfg(feature = "native")]
use crate::adapter::handles::{CallTable, Entry};
use crate::adapter::handles::{Handle, HandleKind};
#[cfg(feature = "native")]
use crate::adapter::typed::{FromApi, IntoApi};
#[cfg(feature = "native")]
use crate::adapter::values::mismatch;
use crate::adapter::values::{handle_to_value, JsonForm};
#[cfg(feature = "native")]
use crate::api;
use crate::api::model::aspects::FixedBlockRole;
use crate::guest::abi;
use crate::guest::lang::rewrites::userdefs::Expand;
use crate::guest::model::aspects::{Referenceable, Sectioning};
use crate::guest::registry;
use crate::runtime::ExtError;

/// Mirrors [`Aspect`](crate::api::model::element::Aspect): a part of an
/// element's behavior, data or a handler object of the guest's traits.
#[derive(Clone)]
pub enum Aspect {
    /// The element's block role.
    BlockRoleAspect(FixedBlockRole),
    /// The element's expansion hook.
    Expand(Arc<dyn Expand>),
    /// What the element offers to references.
    Referenceable(Arc<dyn Referenceable>),
    /// The element as a heading.
    Sectioning(Arc<dyn Sectioning>),
}

/// As the api writes it: a handler object by a placeholder.
impl fmt::Debug for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Aspect::BlockRoleAspect(role) => f.debug_tuple("BlockRoleAspect").field(role).finish(),
            Aspect::Expand(_) => f.debug_tuple("Expand").field(&"…").finish(),
            Aspect::Referenceable(_) => f.debug_tuple("Referenceable").field(&"…").finish(),
            Aspect::Sectioning(_) => f.debug_tuple("Sectioning").field(&"…").finish(),
        }
    }
}

impl Aspect {
    /// Mirrors [`Aspect::kind`](crate::api::model::element::Aspect::kind): the
    /// aspect's kind, as its variant is named.
    pub fn kind(&self) -> &'static str {
        match self {
            Aspect::BlockRoleAspect(_) => "BlockRoleAspect",
            Aspect::Expand(_) => "Expand",
            Aspect::Referenceable(_) => "Referenceable",
            Aspect::Sectioning(_) => "Sectioning",
        }
    }
}

/// An aspect as an element carries it across: its data, or the handle its
/// handler object was lent under in the running call, by
/// [`ElementSpec::with`]. The handle resolves to the host's object while
/// that call runs, when the element's declaration crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AspectForm {
    /// The element's block role.
    BlockRole(FixedBlockRole),
    /// An expansion hook, by its handle.
    Expand(Handle),
    /// What the element offers to references, by its handle.
    Referenceable(Handle),
    /// The element as a heading, by its handle.
    Sectioning(Handle),
}

/// `{"BlockRoleAspect": <block role>}`, `{"Expand": <handle>}`,
/// `{"Referenceable": <handle>}` or `{"Sectioning": <handle>}`; read by the
/// host's `aspect_from_value`.
impl JsonForm for AspectForm {
    fn to_value(&self) -> Value {
        match self {
            AspectForm::BlockRole(role) => json!({ "BlockRoleAspect": role.0.to_value() }),
            AspectForm::Expand(handle) => json!({ "Expand": handle_to_value(*handle) }),
            AspectForm::Referenceable(handle) => json!({ "Referenceable": handle_to_value(*handle) }),
            AspectForm::Sectioning(handle) => json!({ "Sectioning": handle_to_value(*handle) }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an aspect";
        const SHAPE: &str = "a map of one of the keys `BlockRoleAspect`, `Expand`, `Referenceable`, `Sectioning`";
        let mut entries = object(WHAT, value)?.iter();
        let (Some((kind, inner)), None) = (entries.next(), entries.next()) else { return Err(not_shape(WHAT, SHAPE, value)) };
        Ok(match kind.as_str() {
            "BlockRoleAspect" => AspectForm::BlockRole(FixedBlockRole(JsonForm::from_value(inner)?)),
            "Expand" => AspectForm::Expand(abi::handle_of(inner, HandleKind::Expand)?),
            "Referenceable" => AspectForm::Referenceable(abi::handle_of(inner, HandleKind::Referenceable)?),
            "Sectioning" => AspectForm::Sectioning(abi::handle_of(inner, HandleKind::Sectioning)?),
            _ => return Err(not_shape(WHAT, SHAPE, value)),
        })
    }
}

/// The api's aspect, a handler object the entry of its handle, as the host's
/// `aspect_from_value` reads the JSON form with the table at hand.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for AspectForm {
    type Api = api::model::element::Aspect;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        use api::model::element::Aspect as ApiAspect;
        Ok(match self {
            AspectForm::BlockRole(role) => ApiAspect::BlockRoleAspect(role),
            AspectForm::Expand(handle) => match table.entry(handle)? {
                Entry::Expand(hook) => ApiAspect::Expand(Arc::clone(hook)),
                other => return Err(mismatch(HandleKind::Expand, other.kind())),
            },
            AspectForm::Referenceable(handle) => match table.entry(handle)? {
                Entry::Referenceable(aspect) => ApiAspect::Referenceable(Arc::clone(aspect)),
                other => return Err(mismatch(HandleKind::Referenceable, other.kind())),
            },
            AspectForm::Sectioning(handle) => match table.entry(handle)? {
                Entry::Sectioning(aspect) => ApiAspect::Sectioning(Arc::clone(aspect)),
                other => return Err(mismatch(HandleKind::Sectioning, other.kind())),
            },
        })
    }
}

/// The api's aspect `api`, its handler object lent into the table, as the
/// lending natives lend one sent as JSON.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::model::element::Aspect> for AspectForm {
    fn from_api(api: api::model::element::Aspect, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        use api::model::element::Aspect as ApiAspect;
        Ok(match api {
            ApiAspect::BlockRoleAspect(role) => AspectForm::BlockRole(role),
            ApiAspect::Expand(hook) => AspectForm::Expand(table.lend(Entry::Expand(hook))),
            ApiAspect::Referenceable(aspect) => AspectForm::Referenceable(table.lend(Entry::Referenceable(aspect))),
            ApiAspect::Sectioning(aspect) => AspectForm::Sectioning(table.lend(Entry::Sectioning(aspect))),
        })
    }
}

/// Mirrors [`ElementSpec`](crate::api::model::element::ElementSpec): an
/// element, its documentation, its aspects and its data. It keeps, beside
/// each aspect, its crossing form (`AspectForm`: its data, or the handle
/// its lending answered), which is what crosses; a spec read from its
/// crossing form keeps those forms, not the objects.
#[derive(Clone)]
pub struct ElementSpec {
    /// The element's name.
    pub name: String,
    /// Its documentation.
    pub docs: String,
    /// Its aspects, as attached.
    pub(crate) aspects: Vec<Aspect>,
    /// Its data.
    pub value: Option<Value>,
    /// The crossing form of each aspect, in the order attached.
    pub(crate) crossing: Vec<AspectForm>,
}

impl ElementSpec {
    /// Mirrors [`ElementSpec::new`](crate::api::model::element::ElementSpec::new):
    /// the element `name`, with no aspect.
    pub fn new(name: impl Into<String>) -> ElementSpec {
        ElementSpec { name: name.into(), docs: String::new(), aspects: Vec::new(), value: None, crossing: Vec::new() }
    }

    /// Mirrors [`ElementSpec::docs`](crate::api::model::element::ElementSpec::docs).
    pub fn docs(mut self, docs: impl Into<String>) -> ElementSpec {
        self.docs = docs.into();
        self
    }

    /// Mirrors [`ElementSpec::with`](crate::api::model::element::ElementSpec::with):
    /// attaches an aspect. A handler object in it (an expansion hook, a
    /// `Referenceable`, a `Sectioning`) is recorded under the element's name
    /// in the running `register`'s record and lent into the running call, in
    /// every run of `register`; the aspect then crosses as the handle its
    /// lending answered.
    ///
    /// # Errors
    ///
    /// An aspect of the same kind is already attached ([`ExtError::Argument`]);
    /// a handler object given while no `register` runs
    /// ([`ExtError::Handler`]); the host's refusal of its lending.
    pub fn with(mut self, aspect: Aspect) -> Result<ElementSpec, ExtError> {
        if self.aspects.iter().any(|attached| attached.kind() == aspect.kind()) {
            return Err(ExtError::Argument {
                method: "ElementSpec::with".to_string(),
                detail: format!("the aspect `{}` is attached twice to the element `{}`", aspect.kind(), self.name),
            });
        }
        let crossing = registry::record_lent_aspect(&self.name, &aspect)?.form(&self.name)?;
        self.aspects.push(aspect);
        self.crossing.push(crossing);
        Ok(self)
    }

    /// Mirrors [`ElementSpec::with_value`](crate::api::model::element::ElementSpec::with_value).
    pub fn with_value(mut self, value: Value) -> ElementSpec {
        self.value = Some(value);
        self
    }

    /// Mirrors [`ElementSpec::aspects`](crate::api::model::element::ElementSpec::aspects).
    pub fn aspects(&self) -> &[Aspect] {
        &self.aspects
    }
}

/// As the api writes it, each aspect as [`Aspect`]'s `Debug` writes it.
impl fmt::Debug for ElementSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementSpec")
            .field("name", &self.name)
            .field("docs", &self.docs)
            .field("aspects", &self.aspects)
            .field("value", &self.value)
            .finish()
    }
}

/// `{"name", "docs", "aspects": [crossing form, …], "value"}`; read by the
/// host's `element_spec_from_value`.
impl JsonForm for ElementSpec {
    fn to_value(&self) -> Value {
        json!({ "name": self.name, "docs": self.docs, "aspects": self.crossing.to_value(), "value": self.value })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an element";
        let map = object(WHAT, value)?;
        Ok(ElementSpec {
            name: get(WHAT, map, "name")?,
            docs: get::<Option<String>>(WHAT, map, "docs")?.unwrap_or_default(),
            aspects: Vec::new(),
            value: get(WHAT, map, "value")?,
            crossing: get::<Option<Vec<AspectForm>>>(WHAT, map, "aspects")?.unwrap_or_default(),
        })
    }
}

/// The api's element, each aspect its crossing form's, a handler object
/// taken from the table, as the host's `element_spec_from_value` reads the
/// JSON form.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &ElementSpec {
    type Api = api::model::element::ElementSpec;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        let mut spec = api::model::element::ElementSpec::new(self.name.clone()).docs(self.docs.clone());
        for crossing in &self.crossing {
            spec = spec.with(crossing.into_api(table)?)?;
        }
        if let Some(value) = &self.value {
            spec = spec.with_value(value.clone());
        }
        Ok(spec)
    }
}
