//! The implementation of [`ElementResolver`] for [`FlmRegistry`]: looking an
//! element up by its type name when a stored tree is read back.
//!
//! A tree written to its stored form does not carry the definitions of its
//! elements. Most nodes are stored with enough data to rebuild their element,
//! but a node whose element is a synthesized element or a presentation element
//! is stored with the element's type name alone. Reading such a node back
//! needs a lookup from type name to element value. The reading side takes that
//! lookup as an [`ElementResolver`] trait object; this module makes every
//! [`FlmRegistry`] one, so that a tree can be read back against the registry it
//! was produced under.
//!
//! The lookup is [`FlmRegistry::element_value`]. It returns the element value
//! when exactly one value carries the type name, nothing when none does, and
//! the error [`AmbiguousElement`] when several do; the error lists every
//! feature that registered a value under the name. Two features may register
//! elements of the same type name, and then the name does not identify one
//! element. Reading refuses such a node instead of taking the first value,
//! because a wrong element would produce an incorrect document whose damage
//! would be found much later.

use std::sync::Arc;

use crate::model::element::{AmbiguousElement, ElementSpec};
use crate::core::registry::frozen::FlmRegistry;
use crate::lang::serialize::ElementResolver;

impl ElementResolver for FlmRegistry {
    fn resolve_element(&self, name: &str) -> Result<Option<Arc<ElementSpec>>, AmbiguousElement> {
        self.element_value(name).map(|value| value.cloned())
    }
}
