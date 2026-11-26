//! The functions that build the element of a field declared in a scoped
//! package and that read a field node.
//!
//! A [field](crate::guide::concepts_overview#fields-data-a-construct-reads) is a
//! macro that a construct absorbs as data rather than as content: the
//! `\label{sec:intro}` written after `\section{Introduction}`, and the `\label`,
//! `\tag`, `\nonumber` and `\\` written inside the body of a display-math
//! environment. A field says something about the construct that reads it (this
//! heading carries this label, this equation line is not numbered) and is not
//! content at the place where it is written.
//!
//! Every field node is a macro node that carries an element named
//! [`FIELD`] (`flm.field`), and the node's macro name is the field's name as
//! written. Nothing in the library interprets a field by its name across the
//! whole document: the construct that declared the field is the only reader
//! of it, and it reads it by the name it declared ([`field_name`],
//! [`is_field_named`]).
//!
//! A field node that reaches a backend's lowering means that a construct did not
//! read its data, which is a fault of that construct. The only lowering rule of
//! the field element therefore reports an error on every backend
//! ([`FieldRule`](crate::stdlib::backends::lowering::FieldRule)).
//!
//! Fields are declared in two ways:
//!
//! - as [tack-on fields](super::TackOnField) of a tack-on argument
//!   ([`ArgStdKind::TackOn`](super::ArgStdKind::TackOn)), the argument that
//!   absorbs the field macros written at its position in a call, after the
//!   arguments declared before it; this is how `\label` is written after a
//!   heading. A tack-on field is data only: its node carries the language's
//!   own field element, the [structural element](crate::lang::structural)
//!   [`FIELD`];
//! - as ordinary macro declarations of a [scoped
//!   package](super::ScopedPackageDef) whose element is built with
//!   [`field_element`]; this is how the `math` feature declares the four
//!   commands of an equation line inside a display-math environment's body.

// One element serves every field, rather than one per field name chosen by the
// declarer: an element is what a node means to the whole library, and a field means
// something only to its construct, so a definition's own `\label` field is never taken
// for the label of the references feature.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_of};
use crate::lang::flm::Flm;
use crate::lang::structural::FIELD;

/// Creates the element of a field that a scoped package declares as one of its
/// macros: an element named [`FIELD`], documented with `docs`.
///
/// `docs` is the element's documentation; pass what the field is for. The
/// element's block role is the default one
/// ([`BlockRole::DEFAULT`](crate::model::aspects::BlockRole::DEFAULT)), as
/// for the language's own field element. A [tack-on
/// field](super::TackOnField) needs no element: its node carries the
/// language's own.
pub fn field_element(docs: impl Into<String>) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(FIELD)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

/// Returns the field name of `node`, the name its macro is written with
/// (`label` for a `\label` field node), or `None` when `node` is not a field
/// node.
///
/// A construct uses it to pick its own fields out of the nodes it absorbed, by
/// comparing the name with the names it declared. [`is_field_named`] makes the
/// comparison for one name.
pub fn field_name<'t>(node: &NodeRef<'t, Flm>) -> Option<&'t str> {
    if element_of(node).name != FIELD {
        return None;
    }
    node.name()
}

/// Returns whether `node` is a field node written `\name`.
///
/// A construct that reads one field, such as a heading reading its `\label`
/// fields, tests every node it absorbed with this function.
pub fn is_field_named(node: &NodeRef<'_, Flm>, name: &str) -> bool {
    field_name(node) == Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_element_is_named_for_the_field_type() {
        let element = field_element("a label");
        assert_eq!(element.name, FIELD);
        #[cfg(feature = "docs")]
        assert_eq!(element.docs, "a label");
    }
}
