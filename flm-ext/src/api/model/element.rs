//! Mirrors [`flm_core::model::element`]: an element's specification.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::api::lang::rewrites::userdefs::Expand;
use crate::api::model::aspects::{FixedBlockRole, Referenceable, Sectioning};
use crate::runtime::ExtError;

/// Mirrors [`ElementSpec`](flm_core::model::element::ElementSpec): an element's
/// name, documentation, aspects and data.
///
/// What differs: [`with`](ElementSpec::with) takes an [`Aspect`], the aspect
/// named by its variant, and answers an error, not a panic, when the same kind
/// of aspect is attached twice (so the list of aspects is read through
/// [`aspects`](ElementSpec::aspects)); the element's data is a JSON value,
/// which [`Node::element_value`](crate::api::techy::node::Node::element_value)
/// reads back.
#[derive(Clone)]
pub struct ElementSpec {
    /// The element's name.
    pub name: String,
    /// Documentation prose about the element.
    pub docs: String,
    /// The element's aspects, one of each kind at most.
    aspects: Vec<Aspect>,
    /// The element's data.
    pub value: Option<Value>,
}

impl ElementSpec {
    /// Mirrors [`ElementSpec::new`](flm_core::model::element::ElementSpec::new).
    pub fn new(name: impl Into<String>) -> ElementSpec {
        ElementSpec { name: name.into(), docs: String::new(), aspects: Vec::new(), value: None }
    }

    /// Mirrors [`ElementSpec::docs`](flm_core::model::element::ElementSpec::docs).
    pub fn docs(mut self, docs: impl Into<String>) -> ElementSpec {
        self.docs = docs.into();
        self
    }

    /// Mirrors [`ElementSpec::with`](flm_core::model::element::ElementSpec::with):
    /// attaches an aspect. What differs: the aspect is named by its variant
    /// rather than by a type parameter.
    ///
    /// # Errors
    ///
    /// An aspect of the same kind is already attached ([`ExtError::Argument`]).
    pub fn with(mut self, aspect: Aspect) -> Result<ElementSpec, ExtError> {
        if self.aspects.iter().any(|attached| attached.kind() == aspect.kind()) {
            return Err(ExtError::Argument {
                method: "ElementSpec::with".to_string(),
                detail: format!("the aspect `{}` is attached twice to the element `{}`", aspect.kind(), self.name),
            });
        }
        self.aspects.push(aspect);
        Ok(self)
    }

    /// Mirrors [`ElementSpec::with_value`](flm_core::model::element::ElementSpec::with_value):
    /// attaches the element's data. What differs: the data is a JSON value.
    pub fn with_value(mut self, value: Value) -> ElementSpec {
        self.value = Some(value);
        self
    }

    /// No flm-core counterpart: the aspects attached, in the order they were
    /// attached, where `flm-core` has the public field `aspects`; here the list
    /// is private, so that [`with`](ElementSpec::with)'s rule cannot be
    /// bypassed.
    pub fn aspects(&self) -> &[Aspect] {
        &self.aspects
    }
}

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

/// Mirrors the aspect trait objects an element carries in `flm-core`
/// ([`ElementSpec::with`](flm_core::model::element::ElementSpec::with)'s
/// argument): one variant per aspect the extension API knows.
#[derive(Clone)]
pub enum Aspect {
    /// The block role
    /// ([`BlockRoleAspect`](flm_core::model::aspects::BlockRoleAspect)), fixed
    /// for every occurrence of the element.
    BlockRoleAspect(FixedBlockRole),
    /// The expansion hook
    /// ([`Expand`](flm_core::lang::rewrites::userdefs::Expand)) of the
    /// extension API.
    Expand(Arc<dyn Expand>),
    /// The reference points of the element's occurrences
    /// ([`Referenceable`](flm_core::model::aspects::Referenceable)),
    /// answered by a handler of the extension.
    Referenceable(Arc<dyn Referenceable>),
    /// The section the element opens
    /// ([`Sectioning`](flm_core::model::aspects::Sectioning)), answered by a
    /// handler of the extension.
    Sectioning(Arc<dyn Sectioning>),
}

impl Aspect {
    /// No flm-core counterpart: the variant's name, `BlockRoleAspect`,
    /// `Expand`, `Referenceable` or `Sectioning`, the kind of the aspect.
    pub fn kind(&self) -> &'static str {
        match self {
            Aspect::BlockRoleAspect(_) => "BlockRoleAspect",
            Aspect::Expand(_) => "Expand",
            Aspect::Referenceable(_) => "Referenceable",
            Aspect::Sectioning(_) => "Sectioning",
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::model::aspects::BlockRole;

    #[test]
    fn an_aspect_of_one_kind_is_attached_once() {
        let spec = ElementSpec::new("pkg.box").with(Aspect::BlockRoleAspect(FixedBlockRole(BlockRole::Block)));
        let spec = spec.expect("the first block role");
        let error = spec
            .with(Aspect::BlockRoleAspect(FixedBlockRole(BlockRole::DEFAULT)))
            .expect_err("a second block role");
        assert_eq!(error.to_string(), "`ElementSpec::with`: the aspect `BlockRoleAspect` is attached twice to the element `pkg.box`");
    }
}
