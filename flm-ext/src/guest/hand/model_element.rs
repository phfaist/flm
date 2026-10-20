//! The guest's own types of `flm:model/element` that hold handler objects:
//! [`ElementSpec`] and [`Aspect`], with the record-and-lend method
//! [`ElementSpec::with`], the twin of the prelude's fragment
//! `model-element.js`: it records the aspect's handler object under the
//! element and lends it, in every run of `register` (a rehearsal's too).

use std::fmt;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{get, object};
use crate::adapter::values::JsonForm;
use crate::api::model::aspects::FixedBlockRole;
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

/// Mirrors [`ElementSpec`](crate::api::model::element::ElementSpec): an
/// element, its documentation, its aspects and its data. It keeps, beside
/// each aspect, the crossing form its attaching answered (`{"Expand":
/// handle}` for a lent hook), which is what crosses; a spec read from its
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
    pub(crate) crossing: Vec<Value>,
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
    /// in the running `register`'s record and lent, in every run of
    /// `register`; the aspect then crosses as the handle its lending
    /// answered.
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
        let crossing = registry::lend_aspect(&self.name, &aspect)?;
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
        json!({ "name": self.name, "docs": self.docs, "aspects": self.crossing, "value": self.value })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an element";
        let map = object(WHAT, value)?;
        Ok(ElementSpec {
            name: get(WHAT, map, "name")?,
            docs: get::<Option<String>>(WHAT, map, "docs")?.unwrap_or_default(),
            aspects: Vec::new(),
            value: get(WHAT, map, "value")?,
            crossing: get::<Option<Vec<Value>>>(WHAT, map, "aspects")?.unwrap_or_default(),
        })
    }
}
