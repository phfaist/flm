//! The field element `flm.field`, which every field node carries, together with
//! the functions that build it and read it.
//!
//! A [field](crate::guide::concepts_overview#fields-data-a-construct-reads) is a
//! macro that a construct absorbs as data rather than as content: the
//! `\label{sec:intro}` written after `\section{Introduction}`, and the `\label`,
//! `\tag`, `\nonumber` and `\\` written inside the body of a display-math
//! environment. A field says something about the construct that reads it (this
//! heading carries this label, this equation line is not numbered) and is not
//! content at the place where it is written.
//!
//! Every field node carries the element [`FIELD`] (`flm.field`), whose element
//! value is a [`Field`] holding the field's name as written. Nothing in the
//! library interprets a field by its name across the whole document: the
//! construct that declared the field is the only reader of it, and it reads it
//! by the name it declared ([`field_name`], [`is_field_named`]).
//!
//! A field node that reaches a backend's lowering means that a construct did not
//! read its data, which is a fault of that construct. The only lowering rule of
//! the field element therefore reports an error on every backend
//! ([`FieldRule`](crate::stdcore::backends::lowering::FieldRule)).
//!
//! Fields are declared in two ways, and both create their element with
//! [`field_element`]:
//!
//! - as [tack-on fields](super::TackOnField) of a tack-on argument
//!   ([`ArgStdKind::TackOn`](super::ArgStdKind::TackOn)), the trailing argument
//!   that absorbs the field macros written after a construct's other
//!   arguments; this is how `\label` is written after a heading;
//! - as ordinary macro declarations of a [scoped
//!   package](super::ScopedPackageDef) whose element is built with
//!   [`field_element`]; this is how the `math` feature declares the four
//!   commands of an equation line inside a display-math environment's body.

// Why one element for every field, rather than one per field name chosen by
// the declarer: see design rationale at [§flm-dd-dr:lang-field-element].

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::Name;
use crate::lang::flm::Flm;

/// The element name `flm.field`, carried by every field node.
///
/// All fields share this name; the element value, a [`Field`], says which
/// field a node is.
pub const FIELD: &str = "flm.field";

/// The element value of a [`FIELD`] element: the field's name as written,
/// without the escape character (`label`, `tag`, `nonumber`, `\`).
///
/// The node's arguments hold the rest of what was written; this value tells a
/// reader which field the node is. Read the name off a node with
/// [`field_name`], or the whole value with [`element_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The field's name as written after the escape character.
    pub name: Name,
}

/// Creates the element of a field written `\name`: an element named [`FIELD`]
/// whose value is a [`Field`] holding `name`.
///
/// `docs` is the element's documentation; pass what the field is for. The
/// element's block role is the default one
/// ([`BlockRole::DEFAULT`](crate::model::aspects::BlockRole::DEFAULT)).
///
/// [`TackOnField::new`](super::TackOnField::new) calls this for a tack-on
/// field; a feature that declares a field as a macro of a scoped package calls
/// it to build that macro's element.
pub fn field_element(name: impl Into<Name>, docs: impl Into<String>) -> Arc<ElementSpec> {
    let name = name.into();
    Arc::new(
        ElementSpec::new(FIELD)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with_value(Arc::new(Field { name })),
    )
}

/// Creates the element of the field type itself, an element named [`FIELD`]
/// with no value.
///
/// A feature registers this element so that `flm.field` is a declared element
/// type of the registry even when no construct declares a field; the
/// `lowering` feature does so, and registers the element's lowering rule
/// ([`FieldRule`](crate::stdcore::backends::lowering::FieldRule)). No node
/// carries this element: a field node carries the element of its own field,
/// created by [`field_element`], so reading a [`Field`] off this element
/// returns `None`.
pub fn field_type_element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(FIELD)
            .docs(
                "A field: a macro that a construct absorbs as data rather than as content, \
                 such as the `\\label` written after a heading; only the construct that \
                 declared the field reads it.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

/// Returns the field name of `node` (`label` for a `\label` field node), or
/// `None` when `node` is not a field node.
///
/// A construct uses it to pick its own fields out of the nodes it absorbed, by
/// comparing the name with the names it declared. [`is_field_named`] makes the
/// comparison for one name.
pub fn field_name<'t>(node: &NodeRef<'t, Flm>) -> Option<&'t Name> {
    element_value::<Field>(node).map(|field| &field.name)
}

/// Returns whether `node` is a field node written `\name`.
///
/// A construct that reads one field, such as a heading reading its `\label`
/// fields, tests every node it absorbed with this function.
pub fn is_field_named(node: &NodeRef<'_, Flm>, name: &str) -> bool {
    field_name(node).is_some_and(|written| written == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_element_is_named_for_the_field_type_and_valued_with_its_name() {
        let element = field_element("label", "a label");
        assert_eq!(element.name, FIELD);
        assert_eq!(element.value_as::<Field>().expect("valued").name, "label");
        // Two fields of different names are two values of one element name.
        let other = field_element("tag", "a tag");
        assert_eq!(other.name, element.name);
        assert_ne!(other.value_as::<Field>().expect("valued").name, "label");
    }

    #[test]
    fn the_type_element_carries_no_value() {
        let element = field_type_element();
        assert_eq!(element.name, FIELD);
        assert!(element.value_as::<Field>().is_none());
    }
}
