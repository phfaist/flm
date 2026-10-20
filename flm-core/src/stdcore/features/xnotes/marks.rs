//! The fixed marks of the unnumbered xnote kinds, compiled as FLM text once,
//! when the registry is built.
//!
//! An xnote kind's [`mark`](XnoteKind::mark) is one template, read in one way
//! whether the kind is numbered or not: as FLM text in the language of a
//! counter style's number template. A numbered kind's mark is its mark style,
//! which the numbering feature compiles with every declared counter style. An
//! unnumbered kind's mark holds no number placeholder, so it writes the same
//! content beside every xnote: this module compiles that content once, with
//! the same compiler, and the collect stage carries it in the table of
//! xnotes, where [`stage_mark_text`](super::stage_mark_text) reads it
//! ([`Xnotes::fixed_mark`](super::Xnotes::fixed_mark)).
//!
//! The compilation takes two steps of the registry build: a finalizer
//! ([`DeclareFixedMarks`]) reads the service slot of xnote kinds once every
//! contribution is in, and a late-initialization hook (`CompileFixedMarks`)
//! compiles the marks once the language is built.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use techy::core::node::NodeTree;

use crate::core::registry::{BuildError, FeatureBuilder, Finalizer, LateInit, LateInitError};
use crate::lang::rewrites::userdefs::template::TemplateLanguage;
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};
use crate::model::names::Name;
use crate::stdcore::features::numbering::{self, CounterValue, FormatOptions};

use super::config::XnoteKind;
use super::kinds::mark_style;
use super::KINDS_SLOT;

/// The content that each unnumbered xnote kind's fixed mark writes, by the
/// kind's name.
pub(super) type Compiled = Arc<BTreeMap<Name, Vec<Arc<NodeTree<Flm>>>>>;

/// What one registration of the feature holds between the steps of the
/// registry build: the unnumbered xnote kinds with a mark, read from the
/// service slot of xnote kinds once it is assembled, and their marks,
/// compiled once the language is built.
///
/// There is one value per registration, so that the feature registered into
/// two registries keeps the two apart.
#[derive(Default)]
pub(super) struct FixedMarks {
    declared: OnceLock<Vec<XnoteKind>>,
    compiled: OnceLock<Compiled>,
}

impl FixedMarks {
    /// The compiled marks, by the kind's name; empty before the registry is
    /// built, and for a registry holding no unnumbered xnote kind with a
    /// mark.
    pub(super) fn compiled(&self) -> Compiled {
        self.compiled.get().cloned().unwrap_or_default()
    }
}

/// The finalizer (a build hook that runs once every feature has registered):
/// it reads the service slot of xnote kinds, with every contribution in,
/// keeps the unnumbered kinds whose mark is not empty, and registers the
/// late-initialization hook [`CompileFixedMarks`], which compiles their
/// marks.
pub(super) struct DeclareFixedMarks {
    pub(super) marks: Arc<FixedMarks>,
}

impl Finalizer for DeclareFixedMarks {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let refuse = |b: &FeatureBuilder<'_>, detail: &str| BuildError::Feature {
            feature: b.feature().clone(),
            detail: detail.to_string(),
        };
        let Some(services) = b.services() else {
            return Err(refuse(b, "the kinds slot was not assembled before the finalizer ran"));
        };
        let declared: Vec<XnoteKind> = services
            .services_named::<XnoteKind>(KINDS_SLOT)
            .filter(|kind| !kind.numbered && !kind.mark.text().is_empty())
            .cloned()
            .collect();
        if self.marks.declared.set(declared).is_err() {
            return Err(refuse(b, "the feature is registered twice in one registry"));
        }
        b.late_init(Arc::new(CompileFixedMarks { marks: Arc::clone(&self.marks) }))
    }
}

/// The late-initialization hook (a build hook that runs once the language is
/// built): it compiles every unnumbered xnote kind's mark as the kind's mark
/// style, with the compiler of a numbered kind's mark, and writes the content
/// the mark represents, once. A mark that is not valid FLM fails the build:
/// an xnote kind is configuration, and a mistake in it is reported once rather
/// than at every document. A mark that parses with a warning is kept, and the
/// warning is added to the build diagnostics.
// See decision rationale at [§flm-dd-dr:std-xnote-fixed-mark].
struct CompileFixedMarks {
    marks: Arc<FixedMarks>,
}

impl LateInit for CompileFixedMarks {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let Some(declared) = self.marks.declared.get() else { return Ok(()) };
        let mut compiled = BTreeMap::new();
        if !declared.is_empty() {
            let language = TemplateLanguage::new(lang).map_err(|errors| {
                LateInitError::new(format!(
                    "the template language of the xnotes feature could not be built: {}",
                    errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
                ))
            })?;
            for kind in declared {
                let failed = |error: &dyn std::fmt::Display| {
                    LateInitError::new(format!(
                        "the mark `{}` of the xnote kind `{}`: {error}",
                        kind.mark.text(),
                        kind.name
                    ))
                };
                let style =
                    mark_style(kind).compile_with(lang, &language, &[]).map_err(|error| failed(&error))?;
                for warning in style.warnings() {
                    diags.push(warning.clone());
                }
                // A template naming no number writes itself for every number:
                // the one written here stands for all.
                let options = FormatOptions::new().without_word().without_delimiters();
                let segments = numbering::format(&CounterValue::new(1), &style, None, &options)
                    .map_err(|error| failed(&error))?;
                let trees = segments.iter().map(|segment| Arc::clone(segment.content())).collect();
                compiled.insert(kind.name.clone(), trees);
            }
        }
        // The hook runs once per registration of the feature, and a registry
        // holds the feature once, so the cell is empty here.
        let _ = self.marks.compiled.set(Arc::new(compiled));
        Ok(())
    }
}
