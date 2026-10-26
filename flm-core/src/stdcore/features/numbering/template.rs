//! The compilation and instantiation of the numbering feature's templates,
//! which are written in the template language of user definitions and
//! compiled by the language layer's template compiler
//! ([`template`](crate::lang::rewrites::userdefs::template)).
//!
//! Two kinds of text are templates here: a counter style's **number
//! template** in the `Formats` form
//! ([`NumberTemplate::Formats`](super::NumberTemplate::Formats)), which writes
//! the counter's own number (`#{arabic}`, `(#{roman})`), and a counter's
//! **within template**, which writes its ancestors' numbers before the
//! counter's own (`#{flm.headings.section}.`). Both are written in the language
//! of a user definition's template, so markup may surround a substitution
//! (`\textbf{#{arabic}}`) ([the template language shared by
//! features](crate::guide::concepts_overview#the-template-language-shared-by-features)).
//! The single-letter form of a number template
//! ([`NumberTemplate::Initial`](super::NumberTemplate::Initial)) is not a
//! template in this sense; it is compiled without the template compiler.
//!
//! A template that a feature declares is compiled once, when the registry is
//! built. A within template given by a document's numbering options, and a
//! reference point's own number template, are compiled by the numbering stage
//! once per run.
//!
//! The two kinds differ in their vocabulary, the set of names a `#{…}` may
//! contain. The names of a number template are the eight [number
//! formats](super::NumberFormat), and they accept no keys in brackets; the
//! names of a within template are the declared counters and `_parent`, the
//! parent that applies, and each may carry keys in brackets
//! ([`AncestorKeys`]).
//!
//! The functions are crate-private and shared: the headings, lists, theorems,
//! floats, xnotes, citations and defterm features compile and instantiate
//! their own templates (heading title templates, list label templates,
//! theorem heading templates, the defterm lead-in) with them.

use std::sync::Arc;

use techy::core::node::{NodeSlice, NodeTree};
use techy::latexlike::Mode;

use crate::lang::flm::FlmCallableType;
use crate::lang::guard::{on_worker_thread, GuardPolicy};
use crate::lang::rewrites::userdefs::hook::{PlaceholderBindings, PlaceholderValue};
use crate::lang::rewrites::userdefs::instantiate::instantiate;
use crate::lang::rewrites::userdefs::template::{
    compile_template_with, CompiledTemplate, PlaceholderTarget, PlaceholderVocabulary,
    TemplateLanguage, TemplateSpec,
};
use crate::lang::text::is_space;
use crate::lang::{BuiltLang, Flm, FlmSpan};
use crate::model::names::Name;

use super::decl::PARENT_PLACEHOLDER;
use super::formats::NumberFormat;
use super::style::{FormatError, StyleError};

/// Returns the vocabulary of a number template: the eight number formats,
/// none of which accepts keys in brackets.
///
/// Keys in brackets on one of them, such as `#[Roman]{arabic}`, are therefore
/// refused when the template is compiled, so that brackets have one meaning
/// only: the keys of a within template.
pub(crate) fn number_vocabulary() -> PlaceholderVocabulary {
    let mut vocabulary = PlaceholderVocabulary::new(0);
    for format in NumberFormat::ALL {
        vocabulary = vocabulary.extra(format.written_name(), false);
    }
    vocabulary
}

/// Returns the vocabulary of a within template: the counters in `counters`
/// and [`PARENT_PLACEHOLDER`], each of which accepts keys in brackets.
pub(super) fn within_vocabulary<'a>(
    counters: impl Iterator<Item = &'a Name>,
) -> PlaceholderVocabulary {
    let mut vocabulary = PlaceholderVocabulary::new(0).extra(PARENT_PLACEHOLDER, true);
    for counter in counters {
        vocabulary = vocabulary.extra(counter.as_str(), true);
    }
    vocabulary
}

/// Returns `text` compiled against `lang` with the shared template language
/// `language`, accepting the names of `vocabulary`.
///
/// `what` describes the owner of the text in a diagnostic, such as
/// ``the counter `flm.math.equation` ``, as a user definition's diagnostics
/// give the definition the template belongs to.
///
/// # Errors
///
/// [`StyleError::Template`], holding every diagnostic of the compilation, when
/// any of them is an error: a text that is not valid FLM, a `#{…}` whose name
/// is not in the vocabulary, or keys in brackets on a name that accepts none.
pub(crate) fn compile_text(
    text: &str,
    what: &str,
    vocabulary: &PlaceholderVocabulary,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledTemplate, StyleError> {
    let label = format!("template of {what}");
    let spec = TemplateSpec {
        text,
        mode: Mode::Text,
        vocabulary,
        offset: 0,
        definition: what,
        label: &label,
        origin_label: &label,
        user_definition: false,
    };
    compile_template_with(&spec, lang, language).map_err(|diagnostics| StyleError::Template {
        text: text.to_string(),
        diagnostics,
    })
}

/// Returns `template` instantiated as a new tree of content, every
/// placeholder replaced by the content that `bindings` gives for it.
///
/// `site` is the span to which the instance's nodes are attributed: the
/// source of the counter style, since a number is computed instead of being
/// written in a document.
///
/// # Errors
///
/// [`FormatError::NestingTooDeep`] when the content nests too deeply, and
/// [`FormatError::Failed`] for every other failure; such a failure is a fault
/// in the program, since the bindings give content for every name the
/// template was compiled with.
pub(crate) fn instantiate_template(
    site: &FlmSpan,
    template: &CompiledTemplate,
    bindings: &dyn PlaceholderBindings,
) -> Result<Arc<NodeTree<Flm>>, FormatError> {
    instantiate(site, template, bindings, GuardPolicy::StackBudget).map(Arc::new).map_err(
        |error| match error.descent_refusal() {
            Some(detail) => FormatError::NestingTooDeep { detail: detail.to_string() },
            None => FormatError::Failed { detail: error.to_string() },
        },
    )
}

/// Returns the content written by a template that has no placeholder (a
/// fixed list label, the title of a list), instantiated with no binding at
/// the template's own site.
///
/// The instantiation descends recursively, so it runs on a worker thread
/// ([`on_worker_thread`]): on the caller's thread when that thread is already
/// one, as a late-initialization hook's thread is.
///
/// # Errors
///
/// The instantiation's failure, or the failure to create a worker thread, as
/// text.
pub(crate) fn instantiate_fixed(template: &CompiledTemplate) -> Result<Arc<NodeTree<Flm>>, String> {
    let site = template.tree().root().span().clone();
    on_worker_thread(|| {
        instantiate_template(&site, template, &Bindings::new(Vec::new())).map_err(|error| error.to_string())
    })
    .map_err(|error| format!(
        "instantiating a template needs a thread with a large stack, and the operating \
         system refused to create one: {error}"
    ))?
}

/// Returns the names of the placeholders of a compiled template, in the
/// order of the template's tree, each with the text of its keys in brackets
/// (the empty text where it has none).
///
/// The callers read a template's names once, when it is compiled, so that
/// writing a number looks nothing up in a tree.
pub(crate) fn occurrences_of(template: &CompiledTemplate) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for node in template.tree().root().descendants() {
        if node.callable_type() != Some(FlmCallableType::Placeholder) {
            continue;
        }
        let Some(reference) = template.placeholder(node.id()) else { continue };
        let PlaceholderTarget::Extra(name) = &reference.target else { continue };
        let keys = node.argument_content_nodes(0).map(slice_text).unwrap_or_default();
        found.push((name.clone(), keys));
    }
    found
}

/// Returns the byte offset in the template's text at which each placeholder
/// starts, in increasing order.
///
/// A within template is cut into its parts at these positions: everything from
/// one placeholder up to the next is one part, and the text before the first
/// placeholder belongs to the first part.
pub(super) fn placeholder_starts(template: &CompiledTemplate) -> Vec<usize> {
    let mut starts = Vec::new();
    for node in template.tree().root().descendants() {
        if node.callable_type() != Some(FlmCallableType::Placeholder) {
            continue;
        }
        if template.placeholder(node.id()).is_none() {
            continue;
        }
        starts.push(node.span().start());
    }
    starts.sort_unstable();
    starts
}

/// Returns the characters that `nodes` hold, concatenated and trimmed, or the
/// empty text when one of the nodes is not a text node.
///
/// The keys of `#[…]{name}` are read with this function, both when a template
/// is compiled and when it is instantiated: they are a short list of plain
/// words, and anything else is refused as an unknown key.
pub(super) fn slice_text(nodes: NodeSlice<'_, Flm>) -> String {
    let mut text = String::new();
    for node in nodes.iter() {
        match node.chars() {
            Some(chars) => text.push_str(chars),
            None => return String::new(),
        }
    }
    text.trim_matches(is_space).to_string()
}

// --- the keys of a within template's substitution ---------------------------------------------

/// What the keys in brackets of `#[…]{counter}` ask for: the number format in
/// which the ancestor is written, and whether its own ancestors are written
/// before it.
///
/// The keys are separated by commas, and either may be left out:
/// `#{flm.headings.section}` takes the default of both,
/// `#[Roman]{flm.headings.section}` writes the section in capital roman
/// numerals, `#[bare]{flm.headings.section}` writes the section's own number
/// alone, and `#[Roman,bare]{flm.headings.section}` does both.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct AncestorKeys {
    /// The format in which the ancestor's number is written, or `None` for
    /// the format of its own counter style.
    pub(crate) format: Option<NumberFormat>,
    /// Whether the ancestor's own ancestors are left out.
    pub(crate) bare: bool,
}

/// The key that writes an ancestor's own number without its ancestors'.
const BARE_KEY: &str = "bare";

/// Returns the keys that `text` holds.
///
/// # Errors
///
/// The first key that is neither `bare` nor the name of a number format.
pub(crate) fn parse_keys(text: &str) -> Result<AncestorKeys, String> {
    let mut keys = AncestorKeys::default();
    for key in text.split(',') {
        let key = key.trim_matches(is_space);
        if key.is_empty() {
            continue;
        }
        if key == BARE_KEY {
            keys.bare = true;
            continue;
        }
        match NumberFormat::from_written_name(key) {
            Some(format) => keys.format = Some(format),
            None => return Err(key.to_string()),
        }
    }
    Ok(keys)
}

// --- what a template's names are answered with ------------------------------------------------

/// The content with which one `#{…}` of a template is replaced: the name, the
/// text of its keys in brackets, and the content.
pub(crate) struct Binding {
    /// The name between the braces.
    pub(crate) name: String,
    /// The text between the brackets, or the empty text where there is none.
    pub(crate) keys: String,
    /// The content that replaces the placeholder.
    pub(crate) content: Arc<NodeTree<Flm>>,
}

/// The bindings of one instantiation: every name the template contains, with
/// the content computed for it.
///
/// Two occurrences of one name with different keys are two bindings, told
/// apart by the text of their keys, which the placeholder passes back as its
/// substitution argument.
///
/// A name that the template may contain but that has no content at one
/// instantiation (a theorem's title, for a theorem without one) is declared
/// [`absent`](Bindings::absent): it is replaced by nothing, and the
/// conditional `\IfValueT{#{name}}{…}` takes its false branch. Any other name
/// that no binding gives cannot be resolved, and the instantiation fails.
pub(crate) struct Bindings {
    entries: Vec<Binding>,
    absent: Vec<String>,
}

impl Bindings {
    /// Returns the bindings holding `entries`.
    pub(crate) fn new(entries: Vec<Binding>) -> Bindings {
        Bindings { entries, absent: Vec::new() }
    }

    /// Returns these bindings with the name `name` declared to have no
    /// content.
    pub(crate) fn absent(mut self, name: impl Into<String>) -> Bindings {
        self.absent.push(name.into());
        self
    }
}

impl PlaceholderBindings for Bindings {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        let PlaceholderTarget::Extra(name) = target else {
            return PlaceholderValue::Unresolvable;
        };
        let keys = substitution.map(slice_text).unwrap_or_default();
        match self
            .entries
            .iter()
            .find(|entry| entry.name == *name && entry.keys == keys)
        {
            Some(entry) => PlaceholderValue::Tree(&entry.content),
            None if self.absent.iter().any(|absent| absent == name) => PlaceholderValue::NoValue,
            None => PlaceholderValue::Unresolvable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_are_a_format_and_the_bare_mark() {
        assert_eq!(parse_keys(""), Ok(AncestorKeys::default()));
        assert_eq!(
            parse_keys("Roman"),
            Ok(AncestorKeys { format: Some(NumberFormat::CapitalRoman), bare: false })
        );
        assert_eq!(parse_keys("bare"), Ok(AncestorKeys { format: None, bare: true }));
        assert_eq!(
            parse_keys(" Roman , bare "),
            Ok(AncestorKeys { format: Some(NumberFormat::CapitalRoman), bare: true })
        );
    }

    #[test]
    fn a_key_that_is_neither_is_refused() {
        assert_eq!(parse_keys("nosuchkey"), Err("nosuchkey".to_string()));
        assert_eq!(parse_keys("Roman,nosuchkey"), Err("nosuchkey".to_string()));
    }
}
