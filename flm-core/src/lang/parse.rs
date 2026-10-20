//! Parsing FLM source text into a [`Fragment`]: [`parse_fragment`], its
//! options ([`ParseOptions`]) and its errors ([`FragmentError`]).
//!
//! [`parse_fragment`] parses one source with techy's parser under a [built
//! language](BuiltLang), the language [`crate::lang::build`] assembles from the
//! registry's declarations (usually taken from
//! [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang)). It returns
//! a [`Fragment`]: the tree as parsed, the source it came from, and the
//! [diagnostics](crate::guide::concepts_overview#diagnostics) the parse
//! reported. A document made of several files is parsed with
//! [`crate::model::assemble`] instead, which uses the same machinery.
//!
//! A parse is *tolerant*: a problem in the source does not stop it. Each
//! problem becomes a diagnostic stored in the fragment, and the tree still
//! covers the whole input. A parse returns no tree only in the cases that
//! [`FragmentError`] lists.
//!
//! The caller passes a source it has already created and shares
//! (`Arc<FlmSource>`), not a string, because a source carries its
//! [origin](crate::lang::FlmOrigin), and the origin's canonical path is the key
//! of everything keyed by source: detecting a file that includes itself, the
//! tables an embedding program keeps per source. A source created inside the
//! parse call would carry no such key.
//!
//! Every parse runs on a worker thread, a thread with enough stack for the
//! recursion guard's budget. The parse functions check whether the calling
//! thread is a worker thread and, when it is not, run the parse on a new one, so
//! a caller never has to arrange it. See
//! [`on_worker_thread`](crate::lang::guard::on_worker_thread).

use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{ConstructParser, DescentLimitExceeded};
use techy::core::node::BuildId;
use techy::core::{Language, ParsingState};
use techy::error::{Diagnostic, DiagnosticInfo, ParseError};
use techy::source::SourceSpan;

use crate::model::doc::{Fragment, FragmentLayout};
use crate::lang::build::BuiltLang;
use crate::lang::flm::Flm;
use crate::lang::guard::{self, NestingTooDeep};
use crate::lang::input::{InputResolver, Placements};
use crate::lang::origin::{FlmDiagnostic, FlmOrigin, FlmSource};

/// The settings one parse may change relative to the built language it runs
/// under.
///
/// The default (`ParseOptions::default()`) keeps every setting of the built
/// language; a field set here applies to this parse alone.
#[derive(Clone, Default)]
pub struct ParseOptions {
    /// How many diagnostics this parse keeps; `None` keeps the language's own
    /// cap ([`LangInputs::diagnostics_limit`](crate::lang::LangInputs::diagnostics_limit)).
    /// Nothing is lost silently: the count of the diagnostics dropped past the
    /// cap is reported beside the kept ones.
    pub max_diagnostics: Option<usize>,
    /// The input resolver for this parse ([`crate::lang::input`]), used instead
    /// of the language's own; `None` keeps the language's. For example, an
    /// embedding program whose resolver records the files one build reads sets
    /// a new resolver here for that build's parses.
    pub input_resolver: Option<Arc<dyn InputResolver>>,
}

impl fmt::Debug for ParseOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParseOptions")
            .field("max_diagnostics", &self.max_diagnostics)
            .field("input_resolver", &self.input_resolver.as_ref().map(|_| "…"))
            .finish()
    }
}

/// Two options are equal when they set the same cap on diagnostics and the
/// same input resolver (the same shared value, compared by pointer, or none).
impl PartialEq for ParseOptions {
    fn eq(&self, other: &ParseOptions) -> bool {
        self.max_diagnostics == other.max_diagnostics
            && match (&self.input_resolver, &other.input_resolver) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl Eq for ParseOptions {}

/// The reasons a parse returns no tree.
///
/// A tolerant parse turns a problem in the source into a diagnostic and
/// continues, so no variant here is caused by wrong document syntax. Every
/// variant carries what is needed to report the failure, and
/// [`FragmentError::diagnostic`] returns it as the single diagnostic to record
/// in place of the missing tree.
#[derive(Debug)]
pub enum FragmentError {
    /// The recursion guard refused to descend further: the content is nested
    /// too deeply. The diagnostic carries the condition [`NestingTooDeep`], at
    /// the position where the guard refused.
    NestingTooDeep(FlmDiagnostic),
    /// The parse was aborted for a reason no recovery can handle: a parser of
    /// an extension broke a requirement of techy's parser, or a method of the
    /// language failed. This is an error in the program, not in the document.
    Aborted(ParseError<FlmOrigin>),
    /// The parse was not attempted: the calling thread is not a worker thread
    /// and the operating system refused to create one. The diagnostic carries
    /// the condition [`WorkerThreadUnavailable`], at the start of the source.
    WorkerThread(FlmDiagnostic),
}

impl FragmentError {
    /// Returns the single diagnostic, at error severity, that reports the
    /// missing tree: what a run records for a fragment that could not be
    /// parsed.
    ///
    /// For [`NestingTooDeep`](FragmentError::NestingTooDeep) and
    /// [`WorkerThread`](FragmentError::WorkerThread), it is the diagnostic the
    /// variant carries. For [`Aborted`](FragmentError::Aborted), it is built
    /// from the abort: an [`AbortCondition`] holding the abort's identifier and
    /// message, at the abort's span. The abort itself, typed and with techy's
    /// trace of where it happened, remains on the variant.
    pub fn diagnostic(&self) -> FlmDiagnostic {
        match self {
            FragmentError::NestingTooDeep(diagnostic) => diagnostic.clone(),
            FragmentError::Aborted(error) => abort_diagnostic(error),
            FragmentError::WorkerThread(diagnostic) => diagnostic.clone(),
        }
    }
}

impl fmt::Display for FragmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FragmentError::NestingTooDeep(diagnostic) => write!(f, "{}", diagnostic.message()),
            FragmentError::Aborted(error) => write!(f, "the parse was aborted: {}", error.message()),
            FragmentError::WorkerThread(diagnostic) => write!(f, "{}", diagnostic.message()),
        }
    }
}

impl std::error::Error for FragmentError {}

/// Returns the diagnostic that reports an abort: its identifier and message in
/// an [`AbortCondition`], at the abort's span, at error severity.
fn abort_diagnostic(error: &ParseError<FlmOrigin>) -> FlmDiagnostic {
    let condition =
        AbortCondition { identifier: error.identifier().to_string(), message: error.message() };
    Diagnostic::error(condition, error.span().clone())
}

/// Condition `flm.lang.parse-aborted`: a parse was aborted by an error in the
/// program ([`FragmentError::Aborted`]).
///
/// A [condition](crate::guide::concepts_overview#diagnostics) is the typed body
/// of a diagnostic. An abort carries a condition of its own, which cannot be
/// moved into a diagnostic as it is; this condition holds a copy of its
/// identifier and of its message instead. The abort itself, typed and with
/// techy's trace of where it happened, remains on [`FragmentError::Aborted`].
///
/// Reported at error severity, through [`FragmentError::diagnostic`]. Recovery:
/// none; the fragment has no tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.parse-aborted",
    message = "the parse was aborted by an error in the program processing this document ({identifier}): {message}"
)]
pub struct AbortCondition {
    /// The identifier of the abort's condition.
    pub identifier: String,
    /// The message of the abort's condition.
    pub message: String,
}

/// Condition `flm.lang.worker-thread-unavailable`: the parse could not run on
/// a worker thread, so it was not attempted.
///
/// The calling thread was not a worker thread and the operating system refused
/// to create one. The parse is not run on the calling thread instead, because
/// the recursion guard's budget limits how much stack a parse uses and does not
/// check how much stack the thread has: on a thread with less stack, deeply
/// nested content would overflow the stack and end the process instead of
/// being refused with a diagnostic. See [`crate::lang::guard`].
///
/// Reported at error severity ([`FragmentError::WorkerThread`]). Recovery:
/// none; the fragment has no tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.worker-thread-unavailable",
    message = "the parse needs a thread with a large stack, and the operating system \
               refused to create one: {detail}"
)]
pub struct WorkerThreadUnavailable {
    /// The operating system's error message.
    pub detail: String,
}

/// The language value a parse runs with: the built language's, shared, or a
/// copy carrying this parse's own options.
enum LanguageFor<'a> {
    Shared(&'a Language<Flm>),
    Owned(Language<Flm>),
}

impl LanguageFor<'_> {
    fn get(&self) -> &Language<Flm> {
        match self {
            LanguageFor::Shared(language) => language,
            LanguageFor::Owned(language) => language,
        }
    }
}

fn language_for<'a>(lang: &'a BuiltLang, options: &ParseOptions) -> LanguageFor<'a> {
    let limit = match options.max_diagnostics {
        Some(limit) if Some(limit) != lang.driver().diagnostics_limit_setting() => Some(limit),
        _ => None,
    };
    match (limit, &options.input_resolver) {
        (None, None) => LanguageFor::Shared(lang.language()),
        (limit, resolver) => LanguageFor::Owned(lang.language_for_parse(
            limit.or(lang.driver().diagnostics_limit_setting()),
            resolver.clone(),
        )),
    }
}

/// Parses `source` from the parsing state every parse of `lang` starts from
/// ([`BuiltLang::initial_state`]) and returns the resulting fragment.
///
/// `layout` ([`FragmentLayout`]) says how the fragment's top level is laid out
/// (as blocks, inline, or inline until a block appears) when the
/// block-structure rewrite pass runs over it later; it is stored in the
/// fragment and not used by the parse. `options` may
/// change the cap on diagnostics and the input resolver for this parse alone
/// ([`ParseOptions`]).
///
/// The parse is tolerant: a problem in the source becomes a diagnostic of the
/// returned fragment ([`Fragment::diags`]), not an error. The parse runs on a
/// worker thread: on the calling thread when it is one, otherwise on a new
/// thread.
///
/// # Errors
///
/// [`FragmentError::NestingTooDeep`] when the content is nested deeper than the
/// recursion guard allows; [`FragmentError::Aborted`] when an error in the
/// program aborts the parse; [`FragmentError::WorkerThread`] when no worker
/// thread could be created. In each case there is no tree, and
/// [`FragmentError::diagnostic`] returns the diagnostic to record instead.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmOrigin, FlmSource, LangInputs, ParseOptions,
/// };
///
/// // A language with no feature declared at all: FLM's own structure only.
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("one {two} \\emph{three}")
///     .with_origin(FlmOrigin::new("note.flm")));
///
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::default(), &ParseOptions::default(),
/// ).expect("the input is shallow enough to parse");
///
/// // The tree covers the whole input, ...
/// assert!(fragment.tree().root().child_count() > 0);
/// // ... and `\emph`, which no feature here defines, is reported rather than
/// // dropped or fatal.
/// let unresolved = fragment.diags().with_identifier("core.specs.unresolvable-command");
/// assert_eq!(unresolved.count(), 1);
/// ```
///
/// See also [`parse_fragment_with_state`], for a parse that starts from a state
/// derived from the language's initial state, and [`crate::model::assemble`]
/// for a document made of several files.
pub fn parse_fragment(
    lang: &BuiltLang,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    options: &ParseOptions,
) -> Result<Fragment, FragmentError> {
    parse_fragment_with_state(lang, source, layout, options, Arc::clone(lang.initial_state()))
}

/// Parses `source` from the parsing state `state` instead of the language's
/// initial state, and returns the resulting fragment.
///
/// `state` must be derived from the language's initial state with
/// [`BuiltLang::derive_state`], which keeps the features' packages and the
/// structural elements of the language while it changes something else. The
/// library's own caller is the parse of a [user
/// definition](crate::lang::rewrites::userdefs::Definition)'s template, which
/// runs with the [`Template`](crate::lang::RestrictedInterior::Template)
/// restricted-interior policy and the package of placeholders added.
///
/// Everything else (tolerance, the worker thread, `layout` and `options`) is
/// as in [`parse_fragment`], which is this function with the language's initial
/// state.
///
/// # Errors
///
/// The same three as [`parse_fragment`].
pub fn parse_fragment_with_state(
    lang: &BuiltLang,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    options: &ParseOptions,
    state: Arc<ParsingState<Flm>>,
) -> Result<Fragment, FragmentError> {
    let run = || parse_on_worker_thread(lang, Arc::clone(&source), layout, options, state);
    match guard::on_worker_thread(run) {
        Ok(result) => result,
        Err(error) => Err(FragmentError::WorkerThread(Diagnostic::error(
            WorkerThreadUnavailable::new(error.to_string()),
            SourceSpan::new(&source, 0..0),
        ))),
    }
}

/// The parse itself, on a thread that meets the worker-thread requirement of
/// [`crate::lang::guard`].
fn parse_on_worker_thread(
    lang: &BuiltLang,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    options: &ParseOptions,
    state: Arc<ParsingState<Flm>>,
) -> Result<Fragment, FragmentError> {
    let language = language_for(lang, options);
    parse_in(language.get(), source, layout, state, None)
}

/// Returns the language a parse under `options` runs with, its driver carrying
/// `placements`: the language with which the assembly of a document parses
/// each of its files ([`crate::model::assemble`]).
pub(crate) fn language_for_assembly(
    lang: &BuiltLang,
    options: &ParseOptions,
    placements: Arc<Placements>,
) -> Language<Flm> {
    let limit = options.max_diagnostics.or(lang.driver().diagnostics_limit_setting());
    lang.language_recording(limit, options.input_resolver.clone(), Some(placements))
}

/// Parses `source` with `language` from `state`, on a worker thread, with
/// `root` as the root parser when one is given (the language's own
/// otherwise).
///
/// # Errors
///
/// The same three as [`parse_fragment`].
pub(crate) fn parse_with(
    language: &Language<Flm>,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    state: Arc<ParsingState<Flm>>,
    root: Option<&mut (dyn ConstructParser<Flm, Output = BuildId> + Send)>,
) -> Result<Fragment, FragmentError> {
    let run = || parse_in(language, Arc::clone(&source), layout, state, root);
    match guard::on_worker_thread(run) {
        Ok(result) => result,
        Err(error) => Err(FragmentError::WorkerThread(Diagnostic::error(
            WorkerThreadUnavailable::new(error.to_string()),
            SourceSpan::new(&source, 0..0),
        ))),
    }
}

/// The parse itself, on a thread that meets the worker-thread requirement of
/// [`crate::lang::guard`].
fn parse_in(
    language: &Language<Flm>,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    state: Arc<ParsingState<Flm>>,
    root: Option<&mut (dyn ConstructParser<Flm, Output = BuildId> + Send)>,
) -> Result<Fragment, FragmentError> {
    let setup = language.parse_setup(Arc::clone(&source)).with_initial_state(state);
    let result = match root {
        Some(root) => setup.with_root_parser(root).parse(),
        None => setup.parse(),
    };
    match result {
        Ok(result) => Ok(Fragment::new(source, Arc::new(result.tree), result.diagnostics, layout)),
        Err(error) => match error.data().downcast_ref::<DescentLimitExceeded>() {
            Some(refusal) => Err(FragmentError::NestingTooDeep(Diagnostic::error(
                NestingTooDeep::new(refusal.detail.clone()),
                error.span().clone(),
            ))),
            None => Err(FragmentError::Aborted(error)),
        },
    }
}
