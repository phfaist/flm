//! The structural elements of the language, and their stable names.
//!
//! The structural elements are the elements of the node shapes no feature
//! declares: text, comments, containers, groups of each class, paragraphs,
//! paragraph breaks, template placeholders, and the macros and environments a
//! parse could not attribute to any feature (`flm.unknown.macro`,
//! `flm.unknown.environment`); there is one element value per shape. See
//! [structural elements](crate::guide::concepts_overview#structural-elements).
//!
//! Every node of an FLM tree carries an
//! [element](crate::model::element::ElementSpec), the value that says what the
//! node means. A node written as a macro, an environment or a special character
//! sequence of a feature takes its element from the specification it resolved
//! to. Every other node takes its element from the structural elements, so
//! every node has an element and finding it needs no lookup by name.
//!
//! A set of structural elements is one [`StructuralElements`] value. The
//! parsing state carries it in its extension
//! ([`FlmStateExt::structural`](crate::lang::FlmStateExt::structural)), and
//! [`resolve_element`](crate::lang::mint::resolve_element) reads it there. The
//! registry builds the set through [`StructuralElementSpecs`]: it starts from
//! the default set, lets features attach [aspects](crate::model::aspects) to
//! any of the elements (they are ordinary element values), and freezes the
//! result with [`StructuralElements::new`]. The language build then installs
//! the frozen set in the parsing state every parse starts from.
//!
//! The constants of this module ([`TEXT`], [`COMMENT`], …, listed in [`NAMES`])
//! are the elements' stable names: a name identifies the element, and a
//! serialized tree records it.

use std::sync::Arc;

use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::ElementName;

/// The name of the element of a text node, a run of characters: `flm.text`.
pub const TEXT: &str = "flm.text";
/// The name of the element of a source comment node: `flm.comment`.
pub const COMMENT: &str = "flm.comment";
/// The name of the element of a list node, a container that holds content
/// without meaning anything itself (a tree's root, an environment's body, an
/// argument that holds several nodes): `flm.container`.
pub const CONTAINER: &str = "flm.container";
/// The name of the element of a `{…}` content group, and of any group the
/// library creates without a group class: `flm.group`.
pub const GROUP: &str = "flm.group";
/// The name of the element of a mathematics group (`\(…\)`, `\[…\]`, and
/// `$…$` where `$` is enabled as a delimiter): `flm.math`. Whether the group is
/// inline or display mathematics is recorded in the node's group class
/// ([`FlmGroupType::Math`](crate::lang::FlmGroupType::Math)), not in its
/// element.
pub const MATH: &str = "flm.math";
/// The name of the element of a verbatim group, the group node that is the
/// parent of every run of raw text: `flm.verbatim`. See [verbatim
/// regions](crate::guide::concepts_overview#verbatim-regions).
pub const VERBATIM: &str = "flm.verbatim";
/// The name of the element of an opaque wrapper, the group with no delimiters
/// of its own that holds parsed content kept as written
/// ([`FlmGroupType::OpaqueWrapper`](crate::lang::FlmGroupType::OpaqueWrapper)):
/// `flm.opaque-wrapper`. Every backend writes it natively, through its
/// children.
pub const OPAQUE_WRAPPER: &str = "flm.opaque-wrapper";
/// The name of the element of a paragraph: `flm.paragraph`. Paragraphs are
/// created by the block-structure rewrite pass
/// ([`crate::lang::rewrites::blocks`]); no delimiter in the source opens one.
pub const PARAGRAPH: &str = "flm.paragraph";
/// The name of the element of a paragraph-break node, which the parse creates
/// for a blank line: `flm.parbreak`. The block-structure rewrite pass
/// recognizes the node by this element and splits paragraphs there.
pub const PARBREAK: &str = "flm.parbreak";
/// The name of the element of a placeholder node, the place in a [user
/// definition](crate::lang::rewrites::userdefs::Definition)'s template where the
/// content of one of its arguments is inserted: `flm.placeholder`.
pub const PLACEHOLDER: &str = "flm.placeholder";
/// The name of the element of a macro the parse could not attribute to any
/// feature (`\foo` where nothing defines `foo`): `flm.unknown.macro`. Such a
/// node always comes with an error diagnostic.
///
/// The language's own macro fallback, at the bottom of the scope stack,
/// resolves such a name in every parsing mode; a fallback that a feature
/// declares (for example a mathematics feature's, in mathematics mode) takes
/// precedence wherever one exists. The node takes no argument and records the
/// escape character and the name as written, so that a backend can print the
/// macro back unchanged.
///
/// A document can contain such a node, so this element is among the registry's
/// [elements that can occur in a
/// document](crate::core::registry::FlmRegistry::occurrable_elements), and
/// building a plan requires a lowering rule for it on each backend, like any
/// other element.
pub const UNKNOWN_MACRO: &str = "flm.unknown.macro";
/// The name of the element of an environment the parse could not attribute to
/// any feature (`\begin{foo}…\end{foo}` where nothing defines `foo`):
/// `flm.unknown.environment`. Such a node always comes with an error
/// diagnostic.
///
/// Like an unknown macro ([`UNKNOWN_MACRO`]), such an environment is resolved by
/// the language's own fallback in every parsing mode, below every fallback a
/// feature declares. Its body is parsed and laid out as **block** content,
/// because an environment usually holds blocks, so a blank line inside it
/// becomes a paragraph break and causes no further diagnostic.
///
/// Like [`UNKNOWN_MACRO`], it is among the registry's [elements that can occur
/// in a document](crate::core::registry::FlmRegistry::occurrable_elements) and
/// needs a lowering rule on each backend.
pub const UNKNOWN_ENVIRONMENT: &str = "flm.unknown.environment";

/// Every structural element name, in the fixed order in which the set is
/// listed. [`StructuralElements::all`] returns the elements in the same order.
pub const NAMES: [&str; 12] = [
    TEXT,
    COMMENT,
    CONTAINER,
    GROUP,
    MATH,
    VERBATIM,
    OPAQUE_WRAPPER,
    PARAGRAPH,
    PARBREAK,
    PLACEHOLDER,
    UNKNOWN_MACRO,
    UNKNOWN_ENVIRONMENT,
];

/// The structural elements while they can still be changed: one element value
/// per node shape, to which the registry attaches the features' aspects before
/// it freezes the set into a [`StructuralElements`] with
/// [`StructuralElements::new`].
///
/// [`StructuralElementSpecs::default`] is the set FLM starts from. Change an
/// element with [`get_mut`](Self::get_mut).
#[derive(Debug, Clone)]
pub struct StructuralElementSpecs {
    /// The element of text nodes; inline content. See [`TEXT`].
    pub text: ElementSpec,
    /// The element of source comment nodes; inline content, and they render
    /// nothing. See [`COMMENT`].
    pub comment: ElementSpec,
    /// The element of list nodes, containers that hold content without meaning
    /// anything themselves. See [`CONTAINER`].
    pub container: ElementSpec,
    /// The element of content groups, and of groups the library creates
    /// without a group class; inline content. See [`GROUP`].
    pub group: ElementSpec,
    /// The element of mathematics groups; inline content, display mathematics
    /// included, because a display equation is still part of a paragraph. See
    /// [`MATH`].
    pub math: ElementSpec,
    /// The element of verbatim groups; inline content. See [`VERBATIM`].
    pub verbatim: ElementSpec,
    /// The element of opaque wrappers; inline content, like a verbatim group.
    /// See [`OPAQUE_WRAPPER`].
    pub opaque_wrapper: ElementSpec,
    /// The element of paragraphs; block content. See [`PARAGRAPH`].
    pub paragraph: ElementSpec,
    /// The element of paragraph-break nodes. It has no block role: the node
    /// separates blocks and is not a block. See [`PARBREAK`].
    pub parbreak: ElementSpec,
    /// The element of placeholder nodes; no block role. See [`PLACEHOLDER`].
    pub placeholder: ElementSpec,
    /// The element of a macro no feature defines; inline content. See
    /// [`UNKNOWN_MACRO`].
    pub unknown_macro: ElementSpec,
    /// The element of an environment no feature defines; block content. See
    /// [`UNKNOWN_ENVIRONMENT`].
    pub unknown_environment: ElementSpec,
}

impl StructuralElementSpecs {
    /// Returns the element named `name`, one of [`NAMES`], for modification;
    /// `None` for any other name. The registry attaches a feature's aspects to
    /// a structural element through this method, before the set is frozen.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ElementSpec> {
        Some(match name {
            TEXT => &mut self.text,
            COMMENT => &mut self.comment,
            CONTAINER => &mut self.container,
            GROUP => &mut self.group,
            MATH => &mut self.math,
            VERBATIM => &mut self.verbatim,
            OPAQUE_WRAPPER => &mut self.opaque_wrapper,
            PARAGRAPH => &mut self.paragraph,
            PARBREAK => &mut self.parbreak,
            PLACEHOLDER => &mut self.placeholder,
            UNKNOWN_MACRO => &mut self.unknown_macro,
            UNKNOWN_ENVIRONMENT => &mut self.unknown_environment,
            _ => return None,
        })
    }
}

impl Default for StructuralElementSpecs {
    /// Returns the set FLM starts from, with a block role
    /// ([`BlockRoleAspect`]) attached where the node shape has one.
    ///
    /// Text, comments, groups, mathematics, verbatim groups, opaque wrappers
    /// and a macro no feature defines are inline content. A paragraph and an
    /// environment no feature defines are blocks. The paragraph-break node and
    /// the placeholder have no block role, and neither does the container: the
    /// library asks a
    /// container's children for their roles, not the container, and an element
    /// without the aspect counts as inline content
    /// ([`block_role_of`](crate::model::aspects::block_role_of)).
    fn default() -> StructuralElementSpecs {
        // Aspects are keyed by their trait object type: every attachment names
        // `dyn BlockRoleAspect`, never the concrete implementation.
        let inline = || Arc::new(FixedBlockRole(BlockRole::DEFAULT));
        let block = || Arc::new(FixedBlockRole(BlockRole::Block));
        let element = |name: &str, docs: &str| ElementSpec::new(ElementName::from(name)).docs(docs);
        StructuralElementSpecs {
            text: element(TEXT, "a run of text characters").with::<dyn BlockRoleAspect>(inline()),
            comment: element(COMMENT, "a source comment").with::<dyn BlockRoleAspect>(inline()),
            container: element(
                CONTAINER,
                "a container that holds content without meaning anything itself",
            ),
            group: element(GROUP, "a `{…}` content group").with::<dyn BlockRoleAspect>(inline()),
            math: element(MATH, "a mathematics group").with::<dyn BlockRoleAspect>(inline()),
            verbatim: element(VERBATIM, "a verbatim group, the parent of a run of raw text")
                .with::<dyn BlockRoleAspect>(inline()),
            opaque_wrapper: element(
                OPAQUE_WRAPPER,
                "a group with no delimiters that holds parsed content kept as written",
            )
            .with::<dyn BlockRoleAspect>(inline()),
            paragraph: element(PARAGRAPH, "a paragraph").with::<dyn BlockRoleAspect>(block()),
            parbreak: element(PARBREAK, "a paragraph break (a blank line in the source)"),
            placeholder: element(
                PLACEHOLDER,
                "a placeholder of a template (`#1`, `#{name}`), replaced by content when the \
                 template is expanded",
            ),
            unknown_macro: element(
                UNKNOWN_MACRO,
                "a macro the parse could not attribute to any feature, reported as an error",
            )
            .with::<dyn BlockRoleAspect>(inline()),
            unknown_environment: element(
                UNKNOWN_ENVIRONMENT,
                "an environment the parse could not attribute to any feature, reported as an \
                 error; its body is block content",
            )
            .with::<dyn BlockRoleAspect>(block()),
        }
    }
}

/// A frozen, shared set of structural elements: one element value per node
/// shape that no feature declares.
///
/// Built from [`StructuralElementSpecs`] with [`StructuralElements::new`]. A
/// parsing state carries the set in its extension
/// ([`FlmStateExt::structural`](crate::lang::FlmStateExt::structural)); a
/// registry's set is available as
/// [`BuiltLang::structural`](crate::lang::BuiltLang::structural).
#[derive(Debug)]
pub struct StructuralElements {
    /// See [`StructuralElementSpecs::text`].
    pub text: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::comment`].
    pub comment: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::container`].
    pub container: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::group`].
    pub group: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::math`].
    pub math: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::verbatim`].
    pub verbatim: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::opaque_wrapper`].
    pub opaque_wrapper: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::paragraph`].
    pub paragraph: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::parbreak`].
    pub parbreak: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::placeholder`].
    pub placeholder: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::unknown_macro`].
    pub unknown_macro: Arc<ElementSpec>,
    /// See [`StructuralElementSpecs::unknown_environment`].
    pub unknown_environment: Arc<ElementSpec>,
}

impl StructuralElements {
    /// Freezes `specs` into shared element values. Aspects can no longer be
    /// attached afterwards.
    pub fn new(specs: StructuralElementSpecs) -> StructuralElements {
        StructuralElements {
            text: Arc::new(specs.text),
            comment: Arc::new(specs.comment),
            container: Arc::new(specs.container),
            group: Arc::new(specs.group),
            math: Arc::new(specs.math),
            verbatim: Arc::new(specs.verbatim),
            opaque_wrapper: Arc::new(specs.opaque_wrapper),
            paragraph: Arc::new(specs.paragraph),
            parbreak: Arc::new(specs.parbreak),
            placeholder: Arc::new(specs.placeholder),
            unknown_macro: Arc::new(specs.unknown_macro),
            unknown_environment: Arc::new(specs.unknown_environment),
        }
    }

    /// Returns every structural element, in the same fixed order as [`NAMES`].
    pub fn all(&self) -> [&Arc<ElementSpec>; 12] {
        [
            &self.text,
            &self.comment,
            &self.container,
            &self.group,
            &self.math,
            &self.verbatim,
            &self.opaque_wrapper,
            &self.paragraph,
            &self.parbreak,
            &self.placeholder,
            &self.unknown_macro,
            &self.unknown_environment,
        ]
    }

    /// Returns the structural element named `name`, when `name` is one of
    /// [`NAMES`]; `None` otherwise.
    pub fn by_name(&self, name: &str) -> Option<&Arc<ElementSpec>> {
        self.all().into_iter().find(|element| element.name == name)
    }
}

impl Default for StructuralElements {
    fn default() -> StructuralElements {
        StructuralElements::new(StructuralElementSpecs::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::aspects::BlockRoleAspect;

    #[test]
    fn the_default_set_names_every_shape_in_order() {
        let set = StructuralElements::default();
        let names: Vec<&str> = set.all().iter().map(|element| element.name.as_str()).collect();
        assert_eq!(names, NAMES);
        assert!(set.by_name(PARBREAK).is_some());
        assert!(set.by_name("flm.nope").is_none());
    }

    #[test]
    fn the_paragraph_is_a_block_and_the_marker_carries_no_role() {
        let set = StructuralElements::default();
        assert!(set.paragraph.has_aspect::<dyn BlockRoleAspect>());
        assert!(set.text.has_aspect::<dyn BlockRoleAspect>());
        assert!(!set.parbreak.has_aspect::<dyn BlockRoleAspect>());
    }
}
