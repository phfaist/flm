//! The objects implementing the handler traits for a loaded extension, shared
//! by the bindings: one per trait ([`BindingStage`], [`BindingExpand`],
//! [`BindingLower`], [`BindingRewrite`], [`BindingProvider`],
//! [`BindingTemplate`], [`BindingReferenceable`], [`BindingSectioning`],
//! [`BindingLateInit`], [`BindingFinalizer`]), each found again by its
//! registration and its name (or element, or position) in the instance of
//! whichever thread runs it; a feature's configuration reader
//! ([`read_settings`]), found by its package and feature; and
//! [`HandlerKind`], which handler a call runs,
//! with the objects it lends into the call's table and the entry point it
//! ends in ([`HandlerKind::lend`]).
//!
//! # The replies
//!
//! Each object converts its handler's reply into the API's value in one
//! closure, either form ([`Reply`]): the JSON value of a text binding's reply
//! by the reader of its data form (`expansion_from_value`, …), as a JSON
//! binding's answer is read; the guest's reply of the engine `native` by
//! [`IntoApi`], which resolves the handles it holds in the call's table as the
//! reader resolves those the JSON writes. The guest's routing answers each
//! entry with the reply of its kind, so a typed reply of another kind reaches
//! no object.

use flm_core::progress::Scope;
use serde_json::Value;

use super::{run_handler, EntryArg, EntryCall, Registration, Reply};
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::adapter::aspects::{content_ref_from_value, ref_points_from_value};
use crate::adapter::handles::{CallTable, Entry, LangEntry};
#[cfg(feature = "native")]
use crate::adapter::typed::IntoApi;
#[cfg(any(feature = "quickjs", feature = "wasm"))]
use crate::adapter::values::{
    citation_results_from_value, expansion_from_value, lowering_from_value, read_answer_from_value,
    restage_from_value, template_description_from_value, template_files_from_value,
};
use crate::api::core::data::store::StageOut;
use crate::api::core::lowering::{Lower, LowerCx, Lowerer, Lowering};
use crate::api::core::registry::{FeatureBuilder, Finalizer, LateInit};
use crate::api::core::run::{RewriteCx, StageCx};
use crate::api::core::settings::ReadError;
use crate::api::core::stage::{Needs, RewriteDecl, RewriteStage, Stage, StageDecl};
use crate::api::lang::rewrites::userdefs::{Expand, ExpandCx, ExpandPolicy, Expander, Expansion};
use crate::api::lang::{BuiltLang, FlmDiagnostics};
use crate::api::model::aspects::{ContentRef, RefPoint, Referenceable, Sectioning};
use crate::api::stdlib::features::citations::{CitationProvider, CitationRequest, CitationResult, ProviderCx};
use crate::api::stdlib::features::compose::{TemplateContext, TemplateCx, TemplateDescription, TemplateFile, TemplateModule};
use crate::api::techy::node::Node;
use crate::api::techy::transform::{Restage, Restager};
use crate::sealed::Token;
#[cfg(feature = "native")]
use crate::guest::entry::EntryReply;
use crate::runtime::ExtError;

/// A stage object a script passed to `b.stage(…)`: found again by its
/// package, feature and name, in the engine instance of whichever thread
/// runs it (the rehearsal).
pub(crate) struct BindingStage {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The declaration, read once when the stage was declared.
    pub decl: StageDecl,
    /// The needs, read once when the stage was declared.
    pub needs: Needs,
}

impl Stage for BindingStage {
    fn decl(&self) -> StageDecl {
        self.decl.clone()
    }

    fn needs(&self) -> Needs {
        self.needs.clone()
    }

    fn run<'c>(&self, cx: &'c dyn StageCx, out: &'c mut dyn StageOut) -> Result<(), ExtError> {
        let kind = HandlerKind::Run { name: self.decl.name.clone(), cx, out };
        run_handler(&self.registration, kind, |_, _| Ok(()))
    }
}

/// An expansion hook a script attached to an element: found again by its
/// package, feature and element, in the engine instance of whichever thread
/// runs it.
pub(crate) struct BindingExpand {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The element the hook expands.
    pub element: String,
    /// The hook's policy, read once when the hook was attached.
    pub policy: ExpandPolicy,
}

impl Expand for BindingExpand {
    fn policy(&self) -> ExpandPolicy {
        self.policy
    }

    fn expand<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn ExpandCx,
        xp: &'c mut dyn Expander,
    ) -> Result<Expansion, ExtError> {
        let kind = HandlerKind::Expand { element: self.element.clone(), node, cx, xp };
        run_handler(&self.registration, kind, |table, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => expansion_from_value(table, &reply),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Expansion(expansion)) => expansion.into_api(table),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// The body of a lowering rule a script built (`new LoweringRule(name,
/// element, decl, body, mark)`): found again by its package, feature and
/// rule name, in the engine instance of whichever thread lowers (the
/// rehearsal), and called there under the extension's limits.
///
/// A failure inside the body (a thrown value, an exceeded limit, an instance
/// that cannot be created, a body not found after the rehearsal, an answer
/// that is not a `Lowering`) is the body's failure as [`Lower`] states it:
/// the Error `flm.ext.handler-failed` at the node, whose message names the
/// rule, and a childless `flm.pres.unsupported` stand-in in the node's
/// place, the rest of the tree lowered.
pub(crate) struct BindingLower {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The rule's name.
    pub rule: String,
}

impl Lower for BindingLower {
    fn lower<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn LowerCx,
        lw: &'c mut dyn Lowerer,
    ) -> Result<Lowering, ExtError> {
        let kind = HandlerKind::Lower { rule: self.rule.clone(), node, cx, lw };
        run_handler(&self.registration, kind, |table, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => lowering_from_value(table, &reply),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Lowering(lowering)) => lowering.into_api(table),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// A rewrite stage object a script passed to `b.rewrite_stage(…)`: found
/// again by its package, feature and stage name, in the engine instance of
/// whichever thread rewrites (the rehearsal), as a [`BindingStage`] is; its
/// `restage(node, cx, rs)` is called once per node of the tree, nested calls
/// included (a region operation of `rs` calls it again for the region's
/// nodes, on the same engine instance).
///
/// A failure of the handler (a thrown value, an exceeded limit, an instance
/// that cannot be created, a stage not found after the rehearsal, an answer
/// that is not a `Restage`) is the handler's failure as
/// [`RewriteStage`] states it: the Error `flm.ext.handler-failed` at the
/// node, the node kept.
pub(crate) struct BindingRewrite {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The declaration, read once when the stage was declared.
    pub decl: RewriteDecl,
    /// The needs, read once when the stage was declared.
    pub needs: Needs,
}

impl RewriteStage for BindingRewrite {
    fn decl(&self) -> RewriteDecl {
        self.decl.clone()
    }

    fn needs(&self) -> Needs {
        self.needs.clone()
    }

    fn restage<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c dyn RewriteCx,
        rs: &'c mut dyn Restager,
    ) -> Result<Restage, ExtError> {
        let kind = HandlerKind::Restage { name: self.decl.name.clone(), node, cx, rs };
        run_handler(&self.registration, kind, |table, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => restage_from_value(table, &reply),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Restage(restage)) => restage.into_api(table),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// A citation provider a script contributed (`b.peer_slot(PROVIDERS_SLOT,
/// { name, answer_all(requests, cx) })`): found again by its package,
/// feature and name, in the engine instance of whichever thread runs the
/// retrieve stage (the rehearsal), and called there under the extension's
/// limits.
///
/// A failure of the call (a thrown value, an exceeded limit, an instance
/// that cannot be created, a provider not found after the rehearsal, an
/// answer of the wrong shape) is the provider's failure as the adapter
/// states it: every key of the call answers the error
/// `flm.ext.handler-failed: the citation provider ... failed: ...`, which
/// the retrieve stage reports at each key's first citation; those keys have
/// no entry, and no later provider is asked about them. What the script did
/// before it failed (another table's items, a closure's state) is not undone.
pub(crate) struct BindingProvider {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The provider's name (its object's `name`).
    pub name: String,
}

impl CitationProvider for BindingProvider {
    fn name(&self) -> String {
        self.name.clone()
    }

    /// [`answer_all`](CitationProvider::answer_all) for the one request; an
    /// answer that is not one result is the provider's failure.
    fn answer(&self, request: &CitationRequest, cx: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
        let mut results = self.answer_all(std::slice::from_ref(request), cx)?;
        if results.len() == 1 {
            return Ok(results.remove(0));
        }
        Err(ExtError::Value {
            detail: format!("the answer must hold one result for the one key given, and it holds {}", results.len()),
        })
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &dyn ProviderCx) -> Result<Vec<CitationResult>, ExtError> {
        let kind = HandlerKind::Provider { name: self.name.clone(), requests, cx };
        run_handler(&self.registration, kind, |table, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => citation_results_from_value(table, &reply),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Citations(results)) => results.into_api(table),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// A compose template a script wrote as a module and contributed
/// (`b.peer_slot(TEMPLATES_SLOT, { name, describe, files, render })`): found
/// again by its package, feature and name, in the engine instance of
/// whichever thread runs the composer (the rehearsal), and called there
/// under the extension's limits.
///
/// `describe` runs inside the `register` that contributes the module, on the
/// same instance, and finds the object among what that `register` recorded
/// so far; its failure fails the `register`, hence the registry build. A
/// failure of `files` or `render` (a thrown value, an exceeded limit, an
/// instance that cannot be created, a module not found after the rehearsal,
/// an answer of the wrong shape) is the template's failure: the composer
/// reports `flm.compose.template-failed` with the text
/// `flm.ext.handler-failed: the template module ... failed: ...`, and the
/// output is not made.
pub(crate) struct BindingTemplate {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The module's name (its object's `name`): the template's name.
    pub name: String,
}

impl TemplateModule for BindingTemplate {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn describe(&self, settings: &Value) -> Result<TemplateDescription, ExtError> {
        let kind = HandlerKind::TemplateDescribe { name: self.name.clone(), settings };
        run_handler(&self.registration, kind, |_, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => {
                let who = format!(
                    "the template module `{}` of the feature `{}` of the package `{}` of the extension `{}`",
                    self.name, self.registration.feature, self.registration.package, self.registration.binding.identity().stem
                );
                template_description_from_value(&reply, &who)
            }
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Description(description)) => Ok(description),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }

    fn files(&self, data: &TemplateContext, cx: &dyn TemplateCx) -> Result<Vec<TemplateFile>, ExtError> {
        let kind = HandlerKind::TemplateFiles { name: self.name.clone(), data, cx };
        run_handler(&self.registration, kind, |_, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => template_files_from_value(&reply),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Files(files)) => Ok(files),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }

    fn render(&self, data: &TemplateContext, cx: &dyn TemplateCx) -> Result<String, ExtError> {
        let kind = HandlerKind::TemplateRender { name: self.name.clone(), data, cx };
        run_handler(&self.registration, kind, |_, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(Value::String(page)) => Ok(page),
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(_) => Err(ExtError::Value { detail: "the answer of `render` must be a text".to_string() }),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Page(page)) => Ok(page),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// The aspect `Referenceable` a script attached to an element (through
/// `ElementSpec.with` or `FeatureBuilder.attach`): found again by its
/// package, feature and element, in the engine instance of whichever thread
/// runs the `refpoints` stage (the rehearsal).
///
/// A failure (a thrown value, an exceeded limit, an object not found after
/// the rehearsal, an answer that does not read as a list of reference
/// points) is the handler's failure as [`Referenceable`] states it: the
/// occurrence has no reference point.
pub(crate) struct BindingReferenceable {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The element the aspect is attached to.
    pub element: String,
}

impl Referenceable for BindingReferenceable {
    fn points<'c>(&self, node: &'c dyn Node<'c>) -> Result<Vec<RefPoint<'c>>, ExtError> {
        let kind = HandlerKind::Points { element: self.element.clone(), node };
        run_handler(&self.registration, kind, |table, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => ref_points_from_value(table, &reply),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Points(points)) => points.into_api(table),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// The aspect `Sectioning` a script attached to an element: found again by
/// its package, feature and element, in the engine instance of whichever
/// thread runs the `toc` stage (the rehearsal).
///
/// `in_toc` and `toc_title` are called only when the script's object has
/// them ([`methods`](Self::methods)); otherwise they answer the trait's
/// defaults (`true`, no title) without a call. A failure is the handler's
/// failure as [`Sectioning`] states it.
pub(crate) struct BindingSectioning {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The element the aspect is attached to.
    pub element: String,
    /// The names of the methods the script's object has, recorded when the
    /// object was lent.
    pub methods: Vec<String>,
}

impl BindingSectioning {
    /// Whether the script's object has the method `name`.
    fn has(&self, name: &str) -> bool {
        self.methods.iter().any(|method| method == name)
    }

    /// Runs one method of the script's object on `node`, its reply converted
    /// by `convert`.
    fn ask<'c, R>(
        &self,
        method: SectioningMethod,
        node: &'c dyn Node<'c>,
        convert: impl FnOnce(&mut CallTable<'c>, Reply) -> Result<R, ExtError>,
    ) -> Result<R, ExtError> {
        let kind = HandlerKind::Sectioning { method, element: self.element.clone(), node };
        run_handler(&self.registration, kind, convert)
    }
}

/// The `Sectioning` method a call asks for.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SectioningMethod {
    /// `level(node)`.
    Level,
    /// `in_toc(node)`.
    InToc,
    /// `toc_title(node)`.
    TocTitle,
}

impl SectioningMethod {
    /// The method's name, which is also the prelude's entry point.
    pub(crate) fn name(self) -> &'static str {
        match self {
            SectioningMethod::Level => "level",
            SectioningMethod::InToc => "in_toc",
            SectioningMethod::TocTitle => "toc_title",
        }
    }
}

impl Sectioning for BindingSectioning {
    fn level<'c>(&self, node: &'c dyn Node<'c>) -> Result<u32, ExtError> {
        self.ask(SectioningMethod::Level, node, |_, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => reply
                .as_u64()
                .and_then(|level| u32::try_from(level).ok())
                .ok_or_else(|| ExtError::Value { detail: format!("the level of a section must be a number from 0 up, not `{reply}`") }),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Level(level)) => Ok(level),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }

    fn in_toc<'c>(&self, node: &'c dyn Node<'c>) -> Result<bool, ExtError> {
        if !self.has("in_toc") {
            return Ok(true);
        }
        self.ask(SectioningMethod::InToc, node, |_, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => reply.as_bool().ok_or_else(|| ExtError::Value {
                detail: format!("whether a section is listed in the table of contents must be a boolean, not `{reply}`"),
            }),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Listed(listed)) => Ok(listed),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }

    fn toc_title<'c>(&self, node: &'c dyn Node<'c>) -> Result<Option<ContentRef<'c>>, ExtError> {
        if !self.has("toc_title") {
            return Ok(None);
        }
        self.ask(SectioningMethod::TocTitle, node, |table, reply| match reply {
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(Value::Null) => Ok(None),
            #[cfg(any(feature = "quickjs", feature = "wasm"))]
            Reply::Json(reply) => content_ref_from_value(table, &reply).map(Some),
            #[cfg(feature = "native")]
            Reply::Typed(EntryReply::Title(title)) => title.into_api(table),
            #[cfg(feature = "native")]
            Reply::Typed(_) => other_kind(),
        })
    }
}

/// A late-initialization hook a script registered (`b.late_init(hook)`):
/// found again by its position among the feature's late-initialization hooks,
/// in the engine instance of the building thread.
pub(crate) struct BindingLateInit {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The hook's position among the feature's late-initialization hooks,
    /// counted from zero in the order `register` registered them.
    pub index: usize,
}

impl LateInit for BindingLateInit {
    fn late_init<'c>(&self, lang: &'c dyn BuiltLang, diags: &'c mut dyn FlmDiagnostics) -> Result<(), ExtError> {
        let kind = HandlerKind::LateInit { index: self.index, lang, diags };
        run_handler(&self.registration, kind, |_, _| Ok(()))
    }
}

/// A finalizer a script registered (`b.finalizer(hook)`): found again by its
/// position among the feature's finalizers, in the engine instance of the
/// building thread.
pub(crate) struct BindingFinalizer {
    /// The registration that declared it: the extension, the package, the
    /// feature, the configuration.
    pub registration: Registration,
    /// The finalizer's position among the feature's finalizers, counted from
    /// zero in the order `register` registered them.
    pub index: usize,
}

impl Finalizer for BindingFinalizer {
    fn finalize(&self, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        let kind = HandlerKind::Finalize { index: self.index, b };
        run_handler(&self.registration, kind, |_, _| Ok(()))
    }
}

/// Runs the configuration reader of the feature of `registration` with the
/// configuration written, `settings`
/// ([`ExtRuntime::read_settings`](crate::ExtRuntime::read_settings)): no
/// object is lent and the feature is not rehearsed. The reply is the value
/// kept or the value at fault: from a text binding, its JSON form
/// ([`read_answer_from_value`]); from the engine `native`, the guest's own.
///
/// # Errors
///
/// What [`run_handler`] answers: the reader fails or is missing, or the JSON
/// reply is of another shape.
pub(crate) fn read_settings(registration: &Registration, settings: &Value) -> Result<Result<Value, ReadError>, ExtError> {
    run_handler(registration, HandlerKind::ReadSettings { settings }, |_, reply| match reply {
        #[cfg(any(feature = "quickjs", feature = "wasm"))]
        Reply::Json(reply) => read_answer_from_value(&reply),
        #[cfg(feature = "native")]
        Reply::Typed(EntryReply::Read(read)) => Ok(read),
        #[cfg(feature = "native")]
        Reply::Typed(_) => other_kind(),
    })
}

/// Which handler runs, with the objects lent to it.
pub(crate) enum HandlerKind<'c> {
    /// A feature's `register`, with its settings and the builder.
    Register {
        /// The feature's configuration.
        settings: &'c Value,
        /// The builder.
        b: &'c mut dyn FeatureBuilder,
    },
    /// A feature's configuration reader, with the configuration written.
    ReadSettings {
        /// The configuration written under the feature (`{}` when nothing is
        /// written under the feature, or none of the configuration's own
        /// keys).
        settings: &'c Value,
    },
    /// A stage's `run`.
    Run {
        /// The stage's name.
        name: String,
        /// The stage's context.
        cx: &'c dyn StageCx,
        /// The stage's output.
        out: &'c mut dyn StageOut,
    },
    /// An element's expansion hook.
    Expand {
        /// The element's name.
        element: String,
        /// The use site.
        node: &'c dyn Node<'c>,
        /// The context at the use site.
        cx: &'c mut dyn ExpandCx,
        /// The expander.
        xp: &'c mut dyn Expander,
    },
    /// A lowering rule's body.
    Lower {
        /// The rule's name.
        rule: String,
        /// The node being lowered.
        node: &'c dyn Node<'c>,
        /// The presentation tree under construction.
        cx: &'c mut dyn LowerCx,
        /// The lowerer.
        lw: &'c mut dyn Lowerer,
    },
    /// A rewrite stage's `restage`, for one node.
    Restage {
        /// The stage's name.
        name: String,
        /// The node of the input tree.
        node: &'c dyn Node<'c>,
        /// The rewrite's context.
        cx: &'c dyn RewriteCx,
        /// The restage façade.
        rs: &'c mut dyn Restager,
    },
    /// A citation provider's `answer_all` (or `answer` per request).
    Provider {
        /// The provider's name.
        name: String,
        /// The requests.
        requests: &'c [CitationRequest],
        /// The provider's context.
        cx: &'c dyn ProviderCx,
    },
    /// A template module's `describe`, inside the `register` that
    /// contributes it.
    TemplateDescribe {
        /// The module's name.
        name: String,
        /// The feature's configuration.
        settings: &'c Value,
    },
    /// A template module's `files`.
    TemplateFiles {
        /// The module's name.
        name: String,
        /// The template's data.
        data: &'c TemplateContext,
        /// The module's context.
        cx: &'c dyn TemplateCx,
    },
    /// A template module's `render`.
    TemplateRender {
        /// The module's name.
        name: String,
        /// The template's data.
        data: &'c TemplateContext,
        /// The module's context.
        cx: &'c dyn TemplateCx,
    },
    /// The `points` of an element's `Referenceable`.
    Points {
        /// The element's name.
        element: String,
        /// The occurrence.
        node: &'c dyn Node<'c>,
    },
    /// One method of an element's `Sectioning`.
    Sectioning {
        /// The method.
        method: SectioningMethod,
        /// The element's name.
        element: String,
        /// The occurrence.
        node: &'c dyn Node<'c>,
    },
    /// A late-initialization hook.
    LateInit {
        /// The hook's position among the feature's late-initialization hooks.
        index: usize,
        /// The finished language.
        lang: &'c dyn BuiltLang,
        /// The registry's build diagnostics.
        diags: &'c mut dyn FlmDiagnostics,
    },
    /// A finalizer.
    Finalize {
        /// The finalizer's position among the feature's finalizers.
        index: usize,
        /// The builder of the build's second round.
        b: &'c mut dyn FeatureBuilder,
    },
}

/// The guest's reply of another kind than the entry's. The guest's routing
/// answers each entry with the reply of its kind (`guest::entry`), so no
/// object receives one.
#[cfg(feature = "native")]
fn other_kind<R>() -> Result<R, ExtError> {
    unreachable!("the guest's routing answers each entry with the reply of its kind")
}

impl<'c> HandlerKind<'c> {
    /// The scope of the row the handler works under: its context's, for a stage and a
    /// citation provider; [`Scope::none`] for every other handler, whose context has none.
    pub(crate) fn progress(&self) -> Scope {
        match self {
            HandlerKind::Run { cx, .. } => cx.progress().core_scope(Token::new()),
            HandlerKind::Provider { cx, .. } => cx.progress().core_scope(Token::new()),
            _ => Scope::none(),
        }
    }

    /// Lends the handler's objects into `table`; answers the entry call.
    /// `owner` is "of the feature `F` of the package `P` of the extension `S`".
    pub(crate) fn lend(self, table: &mut CallTable<'c>, owner: &str) -> EntryCall<'c> {
        match self {
            HandlerKind::Register { settings, b } => {
                let b = table.lend(Entry::Builder(b));
                EntryCall {
                    entry: "register",
                    args: vec![EntryArg::Json(settings.clone()), EntryArg::Handle(b)],
                    what: format!("`register` {owner}"),
                }
            }
            HandlerKind::ReadSettings { settings } => EntryCall {
                entry: "read_settings",
                args: vec![EntryArg::Json(settings.clone())],
                what: format!("the configuration reader {owner}"),
            },
            HandlerKind::Run { name, cx, out } => {
                let cx = table.lend(Entry::StageCx(cx));
                let out = table.lend(Entry::StageOut(out));
                let what = format!("the stage `{name}` {owner}");
                EntryCall { entry: "run_stage", args: vec![EntryArg::Text(name), EntryArg::Handle(cx), EntryArg::Handle(out)], what }
            }
            HandlerKind::Expand { element, node, cx, xp } => {
                let node = table.lend(Entry::Node(Box::new(node)));
                let cx = table.lend(Entry::ExpandCx(cx));
                let xp = table.lend(Entry::Expander(xp));
                let what = format!("the expansion hook of the element `{element}` {owner}");
                EntryCall {
                    entry: "expand",
                    args: vec![EntryArg::Text(element), EntryArg::Handle(node), EntryArg::Handle(cx), EntryArg::Handle(xp)],
                    what,
                }
            }
            HandlerKind::Lower { rule, node, cx, lw } => {
                let node = table.lend(Entry::Node(Box::new(node)));
                let cx = table.lend(Entry::LowerCx(cx));
                let lw = table.lend(Entry::Lowerer(lw));
                let what = format!("the lowering rule `{rule}` {owner}");
                EntryCall { entry: "lower", args: vec![EntryArg::Text(rule), EntryArg::Handle(node), EntryArg::Handle(cx), EntryArg::Handle(lw)], what }
            }
            HandlerKind::Restage { name, node, cx, rs } => {
                let node = table.lend(Entry::Node(Box::new(node)));
                let cx = table.lend(Entry::RewriteCx(cx));
                let rs = table.lend(Entry::Restager(rs));
                let what = format!("the rewrite stage `{name}` {owner}");
                EntryCall {
                    entry: "restage",
                    args: vec![EntryArg::Text(name), EntryArg::Handle(node), EntryArg::Handle(cx), EntryArg::Handle(rs)],
                    what,
                }
            }
            HandlerKind::Provider { name, requests, cx } => {
                let cx = table.lend(Entry::ProviderCx(cx));
                let what = format!("the citation provider `{name}` {owner}");
                EntryCall { entry: "answer_all", args: vec![EntryArg::Text(name), EntryArg::CitationRequests(requests), EntryArg::Handle(cx)], what }
            }
            HandlerKind::TemplateDescribe { name, settings } => {
                let what = format!("the template module `{name}` {owner}");
                EntryCall { entry: "template_describe", args: vec![EntryArg::Text(name), EntryArg::Json(settings.clone())], what }
            }
            HandlerKind::TemplateFiles { name, data, cx } => {
                let cx = table.lend(Entry::TemplateCx(cx));
                let what = format!("the template module `{name}` {owner}");
                EntryCall { entry: "template_files", args: vec![EntryArg::Text(name), EntryArg::TemplateContext(data), EntryArg::Handle(cx)], what }
            }
            HandlerKind::TemplateRender { name, data, cx } => {
                let cx = table.lend(Entry::TemplateCx(cx));
                let what = format!("the template module `{name}` {owner}");
                EntryCall { entry: "template_render", args: vec![EntryArg::Text(name), EntryArg::TemplateContext(data), EntryArg::Handle(cx)], what }
            }
            HandlerKind::Points { element, node } => {
                let node = table.lend(Entry::Node(Box::new(node)));
                let what = format!("the aspect `Referenceable` of the element `{element}` {owner}");
                EntryCall { entry: "points", args: vec![EntryArg::Text(element), EntryArg::Handle(node)], what }
            }
            HandlerKind::Sectioning { method, element, node } => {
                let node = table.lend(Entry::Node(Box::new(node)));
                let what = format!("`{}` of the aspect `Sectioning` of the element `{element}` {owner}", method.name());
                EntryCall { entry: method.name(), args: vec![EntryArg::Text(element), EntryArg::Handle(node)], what }
            }
            HandlerKind::LateInit { index, lang, diags } => {
                let lang = table.lend(Entry::BuiltLang(LangEntry::Lent(lang)));
                let diags = table.lend(Entry::FlmDiagnostics(diags));
                let what = format!("the late-initialization hook {index} {owner}");
                EntryCall { entry: "late_init", args: vec![EntryArg::Index(index), EntryArg::Handle(lang), EntryArg::Handle(diags)], what }
            }
            HandlerKind::Finalize { index, b } => {
                let b = table.lend(Entry::Builder(b));
                let what = format!("the finalizer {index} {owner}");
                EntryCall { entry: "finalize", args: vec![EntryArg::Index(index), EntryArg::Handle(b)], what }
            }
        }
    }
}
