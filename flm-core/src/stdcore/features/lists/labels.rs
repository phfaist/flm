//! The compilation of label and reference templates against the registry's
//! parsing language ([`BuiltLang`](crate::lang::BuiltLang)) once it is
//! built. The list kinds' templates are compiled once, when the registry
//! is built, by the build hook [`CompileLabels`]; a list's own
//! templates are compiled when the list is lowered, by its lowering rule,
//! where nothing else compiles them. An unnumbered list kind's reference
//! templates are not compiled: its items have no number for them to write,
//! and nothing reads them.
//!
//! A label template is written in the language of a counter style's number
//! template and compiled by the same compiler, so that a text means the same
//! beside an item of an unnumbered list as in a counter style. This module
//! adds what a list needs to know of a compiled template: whether it writes
//! a number format, and, when it writes none, the content of the fixed label
//! it shows, such as a bullet.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use techy::core::node::NodeTree;

use crate::core::registry::{LateInit, LateInitError};
use crate::lang::rewrites::userdefs::template::{CompiledTemplate, TemplateLanguage};
use crate::lang::guard::{on_worker_thread, GuardPolicy};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};
use crate::model::names::Name;
use crate::stdcore::features::numbering::{
    compile_text, instantiate_fixed, number_vocabulary, occurrences_of, StyleError,
};

use super::config::ListsConfig;

/// A label template compiled against the built parsing language.
pub(super) struct CompiledLabel {
    template: CompiledTemplate,
    names_number: bool,
    /// The content a fixed label writes, instantiated once when the template
    /// is compiled, so that every item, and every reference to one, shares
    /// the same tree; `Ok(None)` for a template that writes a number format;
    /// the instantiation's failure as text.
    fixed: Result<Option<Arc<NodeTree<Flm>>>, String>,
}

impl CompiledLabel {
    /// Compiles `text`, a label template, in the language of a counter
    /// style's number template; `what` describes the template in the
    /// compiler's messages.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of the compiler: a text that is not valid FLM, or a
    /// `#{…}` that is not a number format.
    pub(super) fn compile(
        text: &str,
        what: &str,
        lang: &BuiltLang,
        language: &TemplateLanguage,
    ) -> Result<CompiledLabel, StyleError> {
        let template = compile_text(text, what, &number_vocabulary(), lang, language)?;
        let names_number = !occurrences_of(&template).is_empty();
        let fixed = if names_number { Ok(None) } else { instantiate_fixed(&template).map(Some) };
        Ok(CompiledLabel { template, names_number, fixed })
    }

    /// Whether the template writes a number format, so that it shows a
    /// number rather than a fixed label.
    pub(super) fn names_number(&self) -> bool {
        self.names_number
    }

    /// The compiled template: its tree, its text and its placeholders.
    pub(super) fn template(&self) -> &CompiledTemplate {
        &self.template
    }

    /// The content a fixed label writes, as a
    /// [synthesized value](crate::guide::concepts_overview#synthesized-values)
    /// ready to be spliced: the tree instantiated when the template was compiled, shared
    /// by every caller; `None` for a template that writes a number format.
    ///
    /// # Errors
    ///
    /// The instantiation's failure, as text: the content nests too deeply.
    pub(super) fn fixed(&self) -> Result<Option<Arc<NodeTree<Flm>>>, String> {
        self.fixed.clone()
    }
}

/// The compiled state: the template language, kept to compile the templates
/// that lists write; every list kind's label templates by depth; and the
/// guard policy of the built parsing language, under which an item's reference point
/// copies content.
struct Compiled {
    language: TemplateLanguage,
    kinds: BTreeMap<Name, Vec<CompiledLabel>>,
    guard: GuardPolicy,
}

/// The template of `templates` for depth `depth`, 1 for the outermost, the
/// last serving every deeper depth.
fn at_depth(templates: &[CompiledLabel], depth: usize) -> Option<&CompiledLabel> {
    let index = depth.saturating_sub(1).min(templates.len().checked_sub(1)?);
    templates.get(index)
}

/// Whether `text`, a template as a list writes it, holds a placeholder: a
/// `#` that is neither doubled (`##`, a literal `#`) nor escaped (`\#`). In
/// the template language every other `#` begins a placeholder. The rule that
/// lowers an unnumbered list refuses every template holding one (a number
/// format as [`LabelNamesNumber`](super::diagnostics::LabelNamesNumber), any
/// other placeholder as a template that does not compile,
/// [`BadLabel`](super::diagnostics::BadLabel)) and shows the depth's label
/// instead. An item's reference point has no language to compile the
/// template against, and uses this test to decide whether the label beside
/// its item is the list's own.
// See decision rationale at [§flm-dd-dr:std-list-item-reference].
pub(crate) fn holds_placeholder(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            '#' if chars.next() != Some('#') => return true,
            _ => {}
        }
    }
    false
}

/// The label templates of every list kind of the feature's configuration,
/// compiled by [`CompileLabels`] once the parsing language is built, and the template
/// language in which the list rules compile a list's own templates.
pub(super) struct Labels {
    config: ListsConfig,
    compiled: OnceLock<Compiled>,
}

impl fmt::Debug for Labels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Labels").field("config", &self.config).finish_non_exhaustive()
    }
}

impl Labels {
    /// The templates of `config`, not yet compiled.
    pub(super) fn new(config: ListsConfig) -> Labels {
        Labels { config, compiled: OnceLock::new() }
    }

    /// The compiled label template of the list kind `kind` at depth `depth`
    /// (1 for a list inside no list of its list kind; the last template
    /// serves every deeper list); `None` for a list kind that declares none,
    /// or before the hook ran.
    pub(super) fn of_kind(&self, kind: &Name, depth: usize) -> Option<&CompiledLabel> {
        at_depth(self.compiled.get()?.kinds.get(kind)?, depth)
    }

    /// The template language in which the templates were compiled, and in
    /// which a counter style derived from a list's label template is compiled
    /// too; `None` before the hook ran.
    pub(super) fn language(&self) -> Option<&TemplateLanguage> {
        self.compiled.get().map(|compiled| &compiled.language)
    }

    /// The guard policy of the language the templates were compiled against,
    /// or the default policy before the hook ran.
    pub(super) fn guard(&self) -> GuardPolicy {
        self.compiled.get().map(|compiled| compiled.guard).unwrap_or_default()
    }

    /// `text`, a template a list writes, compiled against `lang`; `what`
    /// describes it in the compiler's message (`the list's label`).
    ///
    /// # Errors
    ///
    /// The compiler's message; or, where the hook has not run, a message
    /// saying so, which cannot happen with a registry that was built.
    pub(super) fn compile(
        &self,
        text: &str,
        what: &str,
        lang: &BuiltLang,
    ) -> Result<CompiledLabel, String> {
        let language =
            self.language().ok_or_else(|| String::from("the list label templates were not compiled"))?;
        CompiledLabel::compile(text, what, lang, language).map_err(|error| error.to_string())
    }
}

/// The build hook ([`LateInit`]) of the feature: once the parsing language
/// ([`BuiltLang`](crate::lang::BuiltLang)) is built,
/// it compiles every list kind's label templates, and a numbered list kind's
/// reference templates. It fails the registry build on a template that does
/// not compile and on a label template of an unnumbered list kind that
/// writes a number format: both are mistakes of configuration, reported once
/// rather than in every document. An unnumbered list kind's reference
/// templates are neither compiled nor checked, because they have no effect;
/// switching a list kind between numbered and unnumbered therefore needs no
/// other change to its configuration.
pub(super) struct CompileLabels {
    pub(super) labels: Arc<Labels>,
}

impl CompileLabels {
    /// The label templates of every configured list kind, compiled against
    /// `lang` in `language`, by list kind; a numbered list kind's reference
    /// templates are compiled too, to be checked. Returns the error of the
    /// first template that does not compile, or of a label template of an
    /// unnumbered list kind that writes a number format.
    // See decision rationale at [§flm-dd-dr:std-item-labels].
    fn compile_kinds(
        &self,
        lang: &BuiltLang,
        language: &TemplateLanguage,
    ) -> Result<BTreeMap<Name, Vec<CompiledLabel>>, LateInitError> {
        let mut kinds = BTreeMap::new();
        for (name, kind) in &self.labels.config.environments {
            let compile_all = |texts: &[String], which: &str| {
                let mut compiled = Vec::with_capacity(texts.len());
                for (index, text) in texts.iter().enumerate() {
                    let what = format!(
                        "the {which} template of the list environment `{name}` at depth {}",
                        index + 1
                    );
                    let label = CompiledLabel::compile(text, &what, lang, language)
                        .map_err(|error| LateInitError::new(format!("{what}: {error}")))?;
                    if label.names_number() && !kind.numbered {
                        return Err(LateInitError::new(format!(
                            "the {which} template `{text}` of the list environment `{name}` at \
                             depth {} must not write a number, since the environment is not \
                             numbered",
                            index + 1
                        )));
                    }
                    compiled.push(label);
                }
                Ok(compiled)
            };
            let labels = compile_all(&kind.labels, "label")?;
            if kind.numbered {
                compile_all(&kind.ref_labels, "reference")?;
            }
            kinds.insert(name.clone(), labels);
        }
        Ok(kinds)
    }
}

impl LateInit for CompileLabels {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "the template language of the lists feature could not be built: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        // Every fixed template is instantiated as it is compiled, which
        // descends recursively: the whole loop runs on one worker thread, rather
        // than one thread per template.
        let kinds = on_worker_thread(|| self.compile_kinds(lang, &language))
            .map_err(|error| LateInitError::new(format!(
                "compiling the list label templates needs a thread with a large stack, and \
                 the operating system refused to create one: {error}"
            )))??;
        // The hook runs once per registration of the feature, and a registry
        // holds the feature once, so the cell is empty here.
        let _ = self.labels.compiled.set(Compiled { language, kinds, guard: lang.guard() });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::holds_placeholder;

    #[test]
    fn a_placeholder_is_a_hash_neither_doubled_nor_escaped() {
        for text in ["#{arabic}", "(#{nosuch})", "#1", "a#", "## #{alph}"] {
            assert!(holds_placeholder(text), "{text}");
        }
        for text in ["", "\u{2022}", "--", "##", "\\#", "(\\#)##"] {
            assert!(!holds_placeholder(text), "{text}");
        }
    }
}
