//! Content handlers: what decides which content a float type accepts.
//!
//! A **content handler** ([`FloatContentHandler`]) is a value with a name that
//! accepts a float's content or refuses it. A float type lists, by name, the
//! content handlers it accepts ([`FloatType::handlers`](super::FloatType::handlers));
//! the float's rule tries them in that order, and the first that accepts the
//! content decides. Content that no handler of the type accepts is reported
//! ([`ContentRefused`](super::ContentRefused)).
//!
//! Content handlers are contributed to the service slot [`CONTENT_SLOT`]
//! (`flm.floats.content`) by the feature that owns the kind of content a
//! handler accepts: the `graphics` feature contributes `includegraphics`,
//! which accepts exactly one image, and the `cells` feature contributes
//! `cells`, which accepts exactly one table of cells. The `floats` feature
//! contributes only `any` ([`AnyContent`]), which accepts any content. A
//! feature contributes its handler as a peer contribution
//! ([`peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)): the
//! registry build drops it with one Note when the `floats` feature is not
//! registered, so the contributing feature builds with or without floats.
//!
//! ```
//! use std::sync::Arc;
//! use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
//! use flm_core::lang::Flm;
//! use flm_core::stdcore::features::floats::{CONTENT_SLOT, FloatContentHandler};
//! use techy::core::node::NodeRef;
//!
//! /// Admits exactly one paragraph.
//! struct OneParagraph;
//!
//! impl FloatContentHandler for OneParagraph {
//!     fn name(&self) -> &str {
//!         "one-paragraph"
//!     }
//!     fn admit(&self, content: &[NodeRef<'_, Flm>]) -> Result<(), String> {
//!         match content {
//!             [one] if one.group_type() == Some(flm_core::lang::FlmGroupType::Paragraph) => Ok(()),
//!             _ => Err("exactly one paragraph".to_string()),
//!         }
//!     }
//! }
//!
//! struct Mine;
//! impl FlmFeature for Mine {
//!     fn info(&self) -> FeatureInfo { FeatureInfo::new("mine", "Mine") }
//!     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
//!         b.peer_slot::<dyn FloatContentHandler>(CONTENT_SLOT, Arc::new(OneParagraph))
//!     }
//! }
//! ```
//!
//! When the registry is built, the feature's build hook reads every
//! contribution of the slot into a [`Handlers`] value that the feature's
//! lowering rules share. The stage [`CHECK_HANDLERS`]
//! (`flm.floats.check-handlers`) reports, once per run, each handler that a
//! float type used in the document lists and that no registered feature
//! contributes ([`MissingHandler`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use techy::core::node::NodeRef;
use techy::error::Diagnostic;

use crate::core::data::store::StageOut;
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder, Finalizer};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{Flm, FlmOrigin, nowhere};
use crate::stdcore::features::index::{ELEMENT_INDEX, walk_values_of};

use super::config::FloatType;
use super::diagnostics::{MissingHandler, StageFailed};
use super::elements::FloatElement;
use super::{ANY_HANDLER, CHECK_HANDLERS, CONTENT_SLOT, TYPES_SLOT};

/// A **content handler**: a value that accepts a float's content or refuses
/// it, contributed to the service slot [`CONTENT_SLOT`] and listed by name by
/// the float types that accept it. See the [module documentation](self).
///
/// The registry build fails when two contributed handlers have the same
/// name.
pub trait FloatContentHandler: Send + Sync {
    /// Returns the handler's name, by which a float type lists it
    /// ([`FloatType::handlers`](super::FloatType::handlers)): `any`,
    /// `includegraphics`, `cells`. Unique among the slot's contributions.
    fn name(&self) -> &str;

    /// Returns `Ok(())` when the handler accepts `content`, the float's
    /// content nodes in order: the direct children of its body and the
    /// paragraphs built there, without the commands `\caption`, `\label` and
    /// `\nonumber`, whitespace and source comments
    /// ([`float_body`](super::float_body)). A paragraph holding one of those
    /// commands beside other content is one content node; the float's rule
    /// leaves the command out when it lowers the paragraph. On the raw tree
    /// that the recomposed LaTeX reads (the frozen tree that the
    /// block-structure pass has not processed; [the raw
    /// tree](mod@crate::stdcore::backends::latex_recomposed::rules#the-raw-tree)),
    /// where no paragraph is built, the
    /// content nodes are the body's direct children without the paragraph
    /// breaks. A handler that counts blocks (one image, one table) therefore
    /// gives the same result on both trees.
    ///
    /// # Errors
    ///
    /// The refusal: a description of what the handler accepts, which
    /// completes the sentence "a ‘figure’ must hold …", for example `exactly
    /// one image`. The float's rule reports the refusals only where no
    /// handler of the type accepts the content.
    fn admit(&self, content: &[NodeRef<'_, Flm>]) -> Result<(), String>;
}

/// The content handler `any` ([`ANY_HANDLER`]), the only one the `floats`
/// feature contributes: it accepts any content, including no content.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnyContent;

impl FloatContentHandler for AnyContent {
    fn name(&self) -> &str {
        ANY_HANDLER
    }

    fn admit(&self, _content: &[NodeRef<'_, Flm>]) -> Result<(), String> {
        Ok(())
    }
}

/// The content handlers of a registry, by name, as the float's rules read
/// them.
///
/// The feature creates one value when it registers and shares it among its
/// lowering rules (the general rule and the rules of the `html` and
/// `latex-recomposed` backends). The feature's build hook (a
/// [`Finalizer`]) fills it once, when the registry is built, from every
/// contribution to the service slot [`CONTENT_SLOT`]. A value created with
/// [`Default`] and never filled holds no handler.
#[derive(Default)]
pub struct Handlers {
    by_name: OnceLock<BTreeMap<String, Arc<dyn FloatContentHandler>>>,
}

impl std::fmt::Debug for Handlers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&String> = self.by_name.get().map(|by| by.keys().collect()).unwrap_or_default();
        f.debug_struct("Handlers").field("by_name", &names).finish()
    }
}

/// Which content handler of a float's type accepted the float's content, or
/// the refusals where none did, as [`Handlers::verdict`] returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The handler of this name, the first of the type's handlers to accept
    /// the content, accepted it.
    Admitted(String),
    /// No handler of the type accepted the content. The list holds the
    /// refusal of each handler that a registered feature contributes, in the
    /// type's order. A handler that no registered feature contributes has no
    /// entry, so the list is empty where none of the type's handlers is
    /// contributed.
    Refused(Vec<String>),
}

impl Handlers {
    /// Returns the handler of the name `name`, where a registered feature
    /// contributes one.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn FloatContentHandler>> {
        self.by_name.get()?.get(name)
    }

    /// Returns how `content` fares against the handlers `names`, tried in
    /// order: the first that accepts it decides. A name that no registered
    /// feature contributes accepts nothing and adds no refusal to the list,
    /// since the document's author cannot write the content it would
    /// accept.
    pub fn verdict(&self, names: &[String], content: &[NodeRef<'_, Flm>]) -> Verdict {
        let mut refusals = Vec::with_capacity(names.len());
        for name in names {
            if let Some(handler) = self.get(name) {
                match handler.admit(content) {
                    Ok(()) => return Verdict::Admitted(name.clone()),
                    Err(refusal) => refusals.push(refusal),
                }
            }
        }
        Verdict::Refused(refusals)
    }
}

/// The feature's build hook: it reads every contribution of the service slot
/// [`CONTENT_SLOT`] into the feature's [`Handlers`], and fails the registry
/// build for two handlers of one name and for two float types of one name in
/// the service slot [`TYPES_SLOT`].
pub(super) struct CollectHandlers {
    pub(super) handlers: Arc<Handlers>,
}

impl Finalizer for CollectHandlers {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let refuse = |b: &FeatureBuilder<'_>, detail: String| BuildError::Feature {
            feature: b.feature().clone(),
            detail,
        };
        let Some(services) = b.services() else {
            return Err(refuse(b, "the content slot must be assembled before the finalizer runs".into()));
        };
        // A type another feature contributed is checked against the others
        // here, once the slot holds them all.
        let mut types = BTreeSet::new();
        for float_type in services.services_named::<FloatType>(TYPES_SLOT) {
            if !types.insert(float_type.name.as_str()) {
                return Err(refuse(
                    b,
                    format!("the float type `{}` may not be declared twice", float_type.name),
                ));
            }
        }
        // The rule keeps the shared values themselves: it has no access to the
        // registry's services while it runs. See decision rationale at
        // [§flm-dd-dr:std-floats-slots].
        let mut by_name: BTreeMap<String, Arc<dyn FloatContentHandler>> = BTreeMap::new();
        for handler in services.shared_named::<dyn FloatContentHandler>(CONTENT_SLOT) {
            let name = handler.name().to_string();
            if by_name.insert(name.clone(), Arc::clone(handler)).is_some() {
                return Err(refuse(
                    b,
                    format!("the float content handler `{name}` may not be contributed twice"),
                ));
            }
        }
        if self.handlers.by_name.set(by_name).is_err() {
            return Err(refuse(b, "the feature may be registered at most once in a registry".into()));
        }
        Ok(())
    }
}

/// The stage [`CHECK_HANDLERS`]
/// (`flm.floats.check-handlers`): a check that runs once per document and
/// reports, once per run, each content handler that a float type used in the
/// document lists and that no registered feature contributes
/// ([`MissingHandler`], a Note, at the first float of the type). Every rule
/// that lowers a float requires it, so it runs wherever a float may be
/// lowered; it is a member of the collection
/// [`flm.validated`](crate::stdcore::VALIDATED), so the `check` workflow
/// ([`CHECK_WORKFLOW`](crate::stdcore::CHECK_WORKFLOW)), which validates a
/// document and writes no output, runs it too. It finds the floats through
/// the element index ([`ElementIndex`](crate::stdcore::features::index::ElementIndex), table [`ELEMENT_INDEX`]).
pub(super) struct CheckHandlers;

impl Stage for CheckHandlers {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK_HANDLERS, Effect::Check, Scope::Document).provides(CHECK_HANDLERS).docs(
            "Reports, once per run, each content handler that a float type used in the \
             document lists and that no registered feature contributes (a Note, at the first \
             float of the type).",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(ELEMENT_INDEX)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => {
                out.diags.push(Diagnostic::error(
                    StageFailed { stage: CHECK_HANDLERS.to_string(), detail: error.to_string() },
                    nowhere(FlmOrigin::none()),
                ));
                return;
            }
        };
        let contributed: BTreeSet<&str> = cx
            .registry()
            .services()
            .services_named::<dyn FloatContentHandler>(CONTENT_SLOT)
            .map(|handler| handler.name())
            .collect();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (_, node, value) in walk_values_of::<FloatElement>(cx, index) {
            let float_type = &value.float_type;
            if !seen.insert(float_type.name.to_string()) {
                continue;
            }
            for handler in &float_type.handlers {
                if !contributed.contains(handler.as_str()) {
                    out.diags.push(Diagnostic::note(
                        MissingHandler {
                            float_type: float_type.name.to_string(),
                            handler: handler.clone(),
                        },
                        node.span().clone(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_any_handler_admits_everything() {
        assert_eq!(AnyContent.name(), "any");
        assert_eq!(AnyContent.admit(&[]), Ok(()));
    }

    #[test]
    fn a_handler_nobody_contributes_admits_nothing() {
        let handlers = Handlers::default();
        let _ = handlers.by_name.set(BTreeMap::from([(
            "any".to_string(),
            Arc::new(AnyContent) as Arc<dyn FloatContentHandler>,
        )]));
        let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        assert_eq!(handlers.verdict(&names(&["cells", "any"]), &[]), Verdict::Admitted("any".into()));
        assert_eq!(handlers.verdict(&names(&["cells"]), &[]), Verdict::Refused(Vec::new()));
    }
}
