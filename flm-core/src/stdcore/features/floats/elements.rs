//! The element of the [`floats`](super) feature and its declarations: the
//! element `flm.floats.float` with one element value per float type
//! ([`FloatElement`]), the environment of each float type, the scoped package
//! of `\caption`, `\label` and `\nonumber`, the counter and counter style of
//! each float type, and the float's reference point ([`float_point`]).
//!
//! [`contribute_type`] makes every declaration a float type needs. The
//! `floats` feature calls it for each float type of its configuration, and
//! another feature calls it to declare a float type of its own, so that every
//! float type is declared in the same way.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::lang::mint::{
    ArgDef, ArgStdKind, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef, field_element,
};
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, BlockRole, BlockRoleAspect, FixedBlockRole, NumberSource,
    PointId, RefPoint, Referenceable,
};
use crate::model::addr::NodeAddr;
use crate::model::element::ElementSpec;
use crate::model::names::{FeatureName, Name, RefKind};
use crate::stdcore::features::numbering::{
    COUNTERS_SLOT, CounterDecl, CounterStyleDecl, JoinDecl, STYLES_SLOT,
};
use crate::stdcore::features::refs;

use super::body::{float_fields, label_uses};
use super::config::FloatType;
use super::{
    ALT_KEY, CAPTION_FIELD, CAPTION_OPTIONS_ARG, CAPTION_TEXT_ARG, FIELDS_PACKAGE, FLOAT,
    NONUMBER_FIELD, TYPES_SLOT, counter_name, style_name,
};

/// The data of one element value of [`FLOAT`] (`flm.floats.float`): the
/// float type whose environment the value declares. There is one element
/// value per float type.
///
/// It implements [`Referenceable`]: every float is one reference point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatElement {
    /// The float type, as configured or contributed.
    pub float_type: Arc<FloatType>,
}

/// Returns the identity of the reference point of the float at `addr`: a
/// float has exactly one point, the first (index 0).
pub fn float_point(addr: NodeAddr) -> PointId {
    PointId { addr, index: 0 }
}

/// One reference point per float ([reference points, labels and
/// anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)):
///
/// - of the reference kind of the float type's name;
/// - carrying the labels written in the float's body, which must be of the
///   type's accepted label types;
/// - numbered in the type's counter and counter style where the type's
///   numbering policy numbers the float;
/// - showing, where the float has no number and has a caption, the caption's
///   alternative text, else the caption's full content where its argument is
///   one group or one token
///   ([`Caption::shown_content`](super::Caption::shown_content));
/// - with the anchor `<type>-<number>` (`figure-1`), else the anchor of its
///   first label (`figure-photo`), else none
///   ([`AnchorPolicy::FromValue`]).
impl Referenceable for FloatElement {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let Some(fields) = float_fields(node) else { return Vec::new() };
        let (labels, malformed) = refs::point_labels(&label_uses(&fields));
        let name = self.float_type.name.as_str();
        let number = fields.numbered.then(|| NumberSource {
            counter: counter_name(name),
            instances: Vec::new(),
            style: style_name(name),
            advance: vec![0],
            number_format: None,
        });
        let content = fields.caption.as_ref().and_then(|caption| caption.shown_content(node));
        vec![RefPoint {
            kind: RefKind::from(name),
            labels,
            computed: 0,
            malformed,
            anchor: AnchorPolicy::FromValue { prefix: name.to_string() },
            number,
            content,
            accepts: AcceptedRefTypes::these(self.float_type.label_types.iter().map(String::as_str)),
        }]
    }
}

/// Returns the element value of the float type `float_type`: a block, whose
/// value is a [`FloatElement`] that declares the float's reference point.
fn float_element(float_type: Arc<FloatType>) -> Arc<ElementSpec> {
    let docs = format!("A float of the type `{}`.", float_type.name);
    let value = Arc::new(FloatElement { float_type });
    Arc::new(
        ElementSpec::new(FLOAT)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
            .with_value(value),
    )
}

/// Returns the declaration of the element type [`FLOAT`] itself, owned by
/// `owner`: an element with no value, which no command writes and no node
/// carries. The feature declares it so that the element its rules lower
/// exists in every registry that holds the feature, including one whose
/// configuration declares no float type.
pub(super) fn float_type_def(owner: &FeatureName) -> ElementDef {
    let element = Arc::new(
        ElementSpec::new(FLOAT)
            .docs(
                "A float: one element value per float type, each written as the environment of \
                 the type's name.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
    );
    ElementDef::new(owner.clone(), Vec::<&str>::new(), SyntaxDef::Synthesized, element)
        .docs("The element of every float, with one element value per float type.")
}

/// Returns the declaration of the environment of `float_type`, owned by
/// `owner`: no argument, and a body laid out in paragraphs and blocks that
/// refers to the scoped package [`FIELDS_PACKAGE`] and to the type's own
/// scoped packages. Available in text mode only.
fn environment_def(owner: &FeatureName, float_type: &Arc<FloatType>) -> ElementDef {
    let name = float_type.name.as_str();
    let mut body = BodyDef::new().blocks().scoped(FIELDS_PACKAGE);
    for package in &float_type.scoped_packages {
        body = body.scoped(package.clone());
    }
    ElementDef::new(
        owner.clone(),
        [name],
        SyntaxDef::Environment { args: Vec::new(), body },
        float_element(Arc::clone(float_type)),
    )
    .docs(format!(
        "`\\begin{{{name}}}…\\caption[alt=…]{{…}}\\label{{{name}:…}}\\end{{{name}}}`: a \
         float of the type `{name}`, its content shown with the line under it."
    ))
}

/// Returns the scoped package [`FIELDS_PACKAGE`] (`flm.floats.fields`),
/// declared by `owner`, which holds the three commands of a float's body,
/// each available in text mode: `\caption[alt=…]{…}`, whose optional argument
/// holds key-value pairs and whose mandatory argument is inline content;
/// `\nonumber`, with no argument; and `\label{…}`, the references feature's
/// own declaration ([`refs::label_package`]), declared here under `owner` so
/// that a label can be written in a float's body whether or not the
/// references feature is registered.
pub(super) fn fields_package(owner: &FeatureName) -> ScopedPackageDef {
    let caption = ElementDef::new(
        owner.clone(),
        [CAPTION_FIELD],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(CAPTION_OPTIONS_ARG, ArgStdKind::KeyVals)
                    .docs(
                        "`alt=…`: the caption's alternative text, which a reference to an \
                         unnumbered float shows.",
                    ),
                ArgDef::named(CAPTION_TEXT_ARG, ArgStdKind::Mandatory)
                    .docs("The caption, shown under the float's content."),
            ],
        },
        field_element(
            CAPTION_FIELD,
            "A `\\caption[alt=…]{…}` in a float's body: the float's caption.",
        ),
    )
    .docs(format!(
        "`\\caption[{ALT_KEY}=…]{{…}}`: the float's caption, shown under its content."
    ));
    let nonumber = ElementDef::new(
        owner.clone(),
        [NONUMBER_FIELD],
        SyntaxDef::Macro { args: Vec::new() },
        field_element(NONUMBER_FIELD, "A `\\nonumber` in a float's body: the float is not numbered."),
    )
    .docs("`\\nonumber`: the float is not numbered.");
    let mut elements = vec![caption, nonumber];
    elements.extend(refs::label_package(owner.clone()).elements);
    ScopedPackageDef {
        owner: owner.clone(),
        name: Name::from(FIELDS_PACKAGE),
        elements,
        docs: String::from(
            "The commands of a float's body, which the float reads: `\\caption[alt=…]{…}`, \
             `\\label{…}` and `\\nonumber`.",
        ),
    }
}

/// Returns the declaration of the counter of `float_type`, `flm.floats.<type>`:
/// shown in the type's counter style, and not numbered within another
/// counter.
fn counter(float_type: &FloatType) -> CounterDecl {
    let name = float_type.name.as_str();
    CounterDecl::new(counter_name(name), style_name(name))
        .docs(format!("The numbered floats of the type `{name}`, in document order."))
}

/// Returns the declaration of the counter style of `float_type`,
/// `flm.floats.style.<type>`: the type's words and number format, no
/// delimiters, and several numbers joined compactly
/// ([`JoinDecl::compact`]).
fn style(float_type: &FloatType) -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(style_name(float_type.name.as_str()));
    style.number_format = float_type.number_format.clone();
    style.prefix_words = float_type.words.clone();
    style.join = JoinDecl::compact();
    style
}

/// Makes every declaration the float type `float_type` needs, on behalf of
/// the feature that `b` registers. The [`Floats`](super::Floats) feature calls
/// it for the float types of its configuration, and another feature calls it
/// for a float type it brings with its own content (for example a `listing`
/// float of a feature for program code), so that every float type is declared
/// in the same way.
///
/// Every declaration is a peer declaration or a peer contribution. A peer
/// declaration ([`PeerName`](crate::core::registry::PeerName)) of an element,
/// element type, table, service slot, scoped package or backend tells the
/// registry build that another feature owns that name; a peer contribution is
/// dropped with one Note when the feature that owns the service slot is not
/// registered ([`core::services`](crate::core::services)). The calling feature
/// therefore builds, and every registry holding it renders, without the
/// `floats` feature or the `numbering` feature. The function makes:
///
/// - the type's environment, one more element value of the element type
///   [`FLOAT`] (`flm.floats.float`), which the `floats` feature declares,
///   attributed to the calling feature
///   ([`peer_element_value`](FeatureBuilder::peer_element_value)): no
///   argument, and a body laid out in paragraphs and blocks that refers to the
///   scoped package [`FIELDS_PACKAGE`] and to the type's
///   [`scoped_packages`](FloatType::scoped_packages). The `floats` feature's
///   rules and stage handle it as they handle the feature's own types;
/// - the contribution of the type to the service slot [`TYPES_SLOT`]
///   (`flm.floats.types`);
/// - the contributions of the counter `flm.floats.<type>` ([`counter_name`])
///   and of the counter style `flm.floats.style.<type>` ([`style_name`]),
///   with the type's words and number format, no delimiters and several
///   numbers joined compactly, to the numbering feature's service slots;
/// - the peer declarations of [`FIELDS_PACKAGE`] and of the type's scoped
///   packages, which the body refers to.
///
/// Without the `floats` feature, the environment is not defined: a document
/// that writes it gets an unknown environment, as it would if no feature
/// declared the type, and a document that does not write it is processed as
/// usual.
///
/// # Errors
///
/// [`BuildError::Feature`] when the type fails [`FloatType::validate`]; the
/// [`BuildError`] of the registry builder when a declaration is made outside
/// the registration round that accepts it (see
/// [`Finalizer`](crate::core::registry::Finalizer)).
pub fn contribute_type(b: &mut FeatureBuilder<'_>, float_type: Arc<FloatType>) -> Result<(), BuildError> {
    let owner = b.feature().clone();
    float_type.validate().map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
    b.peer_slot::<FloatType>(TYPES_SLOT, Arc::clone(&float_type))?;
    b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style(&float_type)))?;
    b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter(&float_type)))?;
    b.peer_scoped_package(
        FIELDS_PACKAGE,
        "the scoped package of the commands of a float's body, which the `floats` feature declares",
    )?;
    for package in &float_type.scoped_packages {
        b.peer_scoped_package(
            package.clone(),
            format!(
                "a scoped package the float type `{}` makes available in its body",
                float_type.name
            ),
        )?;
    }
    b.peer_element_value(environment_def(&owner, &float_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::numbering::{NumberTemplate, PrefixWords};

    #[test]
    fn a_type_declares_its_counter_and_its_style() {
        let figure = FloatType::figure();
        let counter = counter(&figure);
        assert_eq!(counter.name.as_str(), "flm.floats.figure");
        assert_eq!(counter.default_style.as_str(), "flm.floats.style.figure");
        assert!(counter.within.is_none());
        let style = style(&figure);
        assert_eq!(style.name.as_str(), "flm.floats.style.figure");
        assert_eq!(style.prefix_words, figure.words);
        assert_eq!(style.number_format, NumberTemplate::formats("#{arabic}"));
        assert_eq!(style.delimiters.open, "");
        assert_eq!(style.join, JoinDecl::compact());
        assert_ne!(style.prefix_words, PrefixWords::default());
    }

    #[test]
    fn the_fields_package_holds_the_three_fields() {
        let package = fields_package(&FeatureName::from("floats"));
        assert_eq!(package.name.as_str(), FIELDS_PACKAGE);
        let names: Vec<Vec<String>> = package
            .elements
            .iter()
            .map(|element| element.names.iter().map(|name| name.to_string()).collect())
            .collect();
        assert_eq!(names, [["caption"], ["nonumber"], ["label"]]);
    }
}
