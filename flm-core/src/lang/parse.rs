//! This module provides [`parse_fragment`], which parses FLM source text into a
//! [`Fragment`], along with its options ([`ParseOptions`]) and the errors that
//! it returns ([`FragmentError`]).
//!
//! [`parse_fragment`] parses one source with a [built language](BuiltLang): the
//! language that [`crate::lang::build`] creates from the definitions declared in
//! the registry. You usually obtain the built language with
//! [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang). The
//! returned [`Fragment`] contains the parsed tree, the source, and the
//! [diagnostics](crate::guide::concepts_overview#diagnostics) reported during
//! the parse. To parse a document made of several files, use
//! [`crate::model::assemble`].
//!
//! A parse is *tolerant*: the parser reports each problem in the source as a
//! diagnostic, stores the diagnostic in the fragment, and continues. The tree
//! covers the whole input. A parse returns no tree only in the cases listed in
//! [`FragmentError`], and an error in the syntax of the document causes none of
//! these cases.
//!
//! The caller passes the source as an `Arc<FlmSource>`, not as a string. The
//! source contains its [origin](crate::lang::FlmOrigin), and the canonical path
//! of the origin identifies the source. For instance, the parser of `\input`
//! uses the canonical path to detect a file that includes itself, and an
//! embedding program can use it as the key of its own tables of sources.
//!
//! A parse runs on a *worker thread*: a thread whose stack is large enough for
//! the recursion guard. If the calling thread is not a worker thread, the parse
//! functions run the parse on a new worker thread, so the caller does not need
//! to create one. See [`on_worker_thread`](crate::lang::guard::on_worker_thread).

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

/// The settings of one parse that can differ from the settings of the built
/// language.
///
/// `ParseOptions::default()` keeps every setting of the built language. A field
/// set here applies to this parse only.
#[derive(Clone, Default)]
pub struct ParseOptions {
    /// The maximum number of diagnostics that this parse stores. `None` keeps
    /// the cap of the built language
    /// ([`LangInputs::diagnostics_limit`](crate::lang::LangInputs::diagnostics_limit)).
    /// Diagnostics beyond the cap are not stored, but they are counted.
    pub max_diagnostics: Option<usize>,
    /// The input resolver for this parse, used in place of the input resolver
    /// of the built language; see [`crate::lang::input`]. `None` keeps the
    /// input resolver of the built language. For instance, an embedding program
    /// that records the files read during a build can set here an input
    /// resolver that records these files, for the parses of this build.
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

/// Two `ParseOptions` values are equal if they have the same cap on
/// diagnostics and the same input resolver. Two input resolvers are the same
/// if both are `None`, or if both point to the same allocation.
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

/// The error that a parse returns when it cannot produce a tree.
///
/// A parse reports each problem in the source as a diagnostic and continues.
/// An error in the syntax of the document therefore never causes a
/// `FragmentError`. [`FragmentError::diagnostic`] returns a diagnostic that
/// reports the error, to be recorded in place of the missing tree.
#[derive(Debug)]
pub enum FragmentError {
    /// The content is nested too deeply, and the recursion guard stopped the
    /// parse. The diagnostic has the condition [`NestingTooDeep`] and the
    /// position at which the guard stopped the parse.
    NestingTooDeep(FlmDiagnostic),
    /// The parse was aborted because of an error in the program: a parser of an
    /// extension broke a requirement of techy's parser, or a method of the
    /// language failed. The [`ParseError`] is the error with which techy
    /// aborted the parse.
    Aborted(ParseError<FlmOrigin>),
    /// The parse did not run: the calling thread is not a worker thread, and
    /// the operating system failed to create one. The diagnostic has the
    /// condition [`WorkerThreadUnavailable`] and the position of the start of
    /// the source.
    WorkerThread(FlmDiagnostic),
}

impl FragmentError {
    /// Returns the diagnostic, at error severity, that reports this error. A
    /// program records this diagnostic for a fragment that could not be parsed.
    ///
    /// For [`NestingTooDeep`](FragmentError::NestingTooDeep) and
    /// [`WorkerThread`](FragmentError::WorkerThread), returns the diagnostic of
    /// the variant. For [`Aborted`](FragmentError::Aborted), returns a
    /// diagnostic with the condition [`AbortCondition`], at the span of the
    /// [`ParseError`]. This condition contains the identifier and the message
    /// of the `ParseError`.
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
            FragmentError::Aborted(error) => write!(f, "{}", abort_diagnostic(error).message()),
            FragmentError::WorkerThread(diagnostic) => write!(f, "{}", diagnostic.message()),
        }
    }
}

impl std::error::Error for FragmentError {}

/// Returns the diagnostic, at error severity, that reports the abort `error`:
/// a diagnostic with the condition [`AbortCondition`], at the span of `error`.
fn abort_diagnostic(error: &ParseError<FlmOrigin>) -> FlmDiagnostic {
    let condition =
        AbortCondition { identifier: error.identifier().to_string(), message: error.message() };
    Diagnostic::error(condition, error.span().clone())
}

/// The error that is reported when a parse was aborted because of an error in
/// the program (id `flm.lang.parse-aborted`).
///
/// A document cannot cause this error: it is caused by a faulty extension or by
/// a faulty method of the language. [`FragmentError::diagnostic`] reports this
/// error for [`FragmentError::Aborted`]. The fragment has no tree.
///
/// The condition contains a copy of the identifier and of the message of the
/// [`ParseError`] that aborted the parse. The `ParseError` itself, with techy's
/// record of where the abort happened, is available in
/// [`FragmentError::Aborted`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.parse-aborted",
    message = "Parse aborted by a program error (‘{identifier}’): {message}"
)]
pub struct AbortCondition {
    /// The identifier of the condition of the [`ParseError`].
    pub identifier: String,
    /// The message of the [`ParseError`].
    pub message: String,
}

/// The error that is reported when the operating system fails to create a
/// worker thread for a parse (id `flm.lang.worker-thread-unavailable`).
///
/// A parse runs on a worker thread, a thread with a large stack; see
/// [`crate::lang::guard`]. When the calling thread is not a worker thread, the
/// parse functions create one. If the operating system fails to create this
/// thread, the parse does not run, and the parse functions return
/// [`FragmentError::WorkerThread`] with this error. The fragment has no tree.
///
/// In this case, the parse does not run on the calling thread either. The
/// recursion guard limits how deeply a parse descends, but it does not check
/// the size of the stack of the thread. On a thread with a smaller stack,
/// deeply nested content could overflow the stack and end the process.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.worker-thread-unavailable",
    message = "Failed to create a thread with a sufficiently large stack for the parse: {detail}"
)]
pub struct WorkerThreadUnavailable {
    /// The operating system's error message.
    pub detail: String,
}

/// The techy [`Language`] that a parse uses: either the language of the
/// [`BuiltLang`], borrowed, or a copy of it with the options of this parse.
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

/// Parses `source` with the built language `lang`, and returns the resulting
/// fragment.
///
/// The parse starts from the initial parsing state of `lang`
/// ([`BuiltLang::initial_state`]). `layout` ([`FragmentLayout`]) is stored in
/// the fragment and is not used by the parse. It specifies how the
/// block-structure rewrite pass lays out the top level of the fragment: as
/// blocks, as inline content, or as inline content until a block appears.
/// `options` can change the cap on diagnostics and the input resolver for this
/// parse; see [`ParseOptions`].
///
/// The parse is tolerant: each problem in the source is reported as a
/// diagnostic of the returned fragment ([`Fragment::diags`]). The parse runs on
/// a worker thread; see the [module documentation](crate::lang::parse).
///
/// To parse from another parsing state than the initial state, use
/// [`parse_fragment_with_state`]. To parse a document made of several files,
/// use [`crate::model::assemble`].
///
/// # Errors
///
/// Returns an error, and no fragment, in the following cases:
///
/// - [`FragmentError::NestingTooDeep`] if the content is nested more deeply
///   than the recursion guard allows;
/// - [`FragmentError::Aborted`] if an error in the program aborts the parse;
/// - [`FragmentError::WorkerThread`] if the operating system fails to create a
///   worker thread.
///
/// [`FragmentError::diagnostic`] returns the diagnostic to record in place of
/// the fragment.
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
/// // A language without any feature: it contains only FLM's structural
/// // elements.
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
/// // ... and `\emph`, which is not defined in any feature of this language, is
/// // reported as a diagnostic.
/// let unresolved = fragment.diags().with_identifier("core.specs.unresolvable-command");
/// assert_eq!(unresolved.count(), 1);
/// ```
pub fn parse_fragment(
    lang: &BuiltLang,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    options: &ParseOptions,
) -> Result<Fragment, FragmentError> {
    parse_fragment_with_state(lang, source, layout, options, Arc::clone(lang.initial_state()))
}

/// Parses `source` with the built language `lang`, starting from the parsing
/// state `state`, and returns the resulting fragment.
///
/// `state` must be derived from the initial parsing state of `lang` with
/// [`BuiltLang::derive_state`]. A state derived in this way contains the
/// packages of the features and the structural elements of the language. For
/// instance, the library parses the template of a [user
/// definition](crate::lang::rewrites::userdefs::Definition) from a derived
/// state with the [`Template`](crate::lang::RestrictedInterior::Template)
/// policy for restricted interiors and with the package of the placeholders.
///
/// The parameters `layout` and `options` and the behavior of the parse are the
/// same as for [`parse_fragment`]. [`parse_fragment`] calls this function with
/// the initial parsing state of `lang`.
///
/// # Errors
///
/// Returns the same errors as [`parse_fragment`].
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

/// Performs the parse of [`parse_fragment_with_state`]. The calling thread must
/// be a worker thread; see [`crate::lang::guard`].
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

/// Returns the language with which [`crate::model::assemble`] parses each file
/// of a document: the language of `lang` with the options `options`, and with
/// a driver that records the heading inputs in `placements`.
pub(crate) fn language_for_assembly(
    lang: &BuiltLang,
    options: &ParseOptions,
    placements: Arc<Placements>,
) -> Language<Flm> {
    let limit = options.max_diagnostics.or(lang.driver().diagnostics_limit_setting());
    lang.language_recording(limit, options.input_resolver.clone(), Some(placements))
}

/// Parses `source` with `language`, starting from the parsing state `state`,
/// and returns the resulting fragment. If `root` is given, it is the parser of
/// the top level of the source; otherwise, the root parser of `language` is
/// used. Like [`parse_fragment`], this function runs the parse on a worker
/// thread.
///
/// # Errors
///
/// Returns the same errors as [`parse_fragment`].
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

/// Performs the parse of [`parse_with`]. The calling thread must be a worker
/// thread; see [`crate::lang::guard`].
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
