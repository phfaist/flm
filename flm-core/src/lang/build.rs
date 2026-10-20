//! Building the language that every parse of a document runs under:
//! [`build_language`], its inputs ([`LangInputs`]) and its result
//! ([`BuiltLang`]).
//!
//! The registry collects what every [feature](crate::core::registry::FlmFeature)
//! declares and passes it to [`build_language`] as [`LangInputs`].
//! [`build_language`] turns the inputs into a [`BuiltLang`]: one techy
//! `Language` value, the parsing state every parse starts from, and the
//! providers and elements that state refers to. Everything a parse needs is decided here, once;
//! [`crate::lang::parse`] only runs it. A registry holds its built language
//! ([`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang)); a test or a
//! program that needs a language without a registry calls [`build_language`]
//! directly.
//!
//! # What the build does
//!
//! 1. **Builds the packages.** [Minting](crate::lang::mint) builds techy
//!    packages and specifications from the declarations: one package per
//!    declaring feature and per set of parsing modes its declarations are
//!    available in, the scoped packages, the package holding the fallback
//!    specifications, and the package holding the specifications of the field
//!    macros (such as `\label`) that a tack-on argument absorbs. A tack-on
//!    argument is a trailing argument of a construct that absorbs the field
//!    macros written after the construct's other arguments (see [fields: data
//!    a construct reads](crate::guide::concepts_overview#fields-data-a-construct-reads)).
//! 2. **Derives the parsing state every parse starts from**
//!    ([`BuiltLang::initial_state`]) in one step from the language's initial
//!    parsing state (techy's `ParsingState::lang_initial`, built from
//!    [`Flm::initial_state_data`](crate::techy::core::Lang::initial_state_data)):
//!    the changes to the tokenizer rules that the parsing settings
//!    ([`FlmParsing`]) ask for, a state extension holding the registry's
//!    [structural elements](crate::lang::structural) and the parsing settings,
//!    and a new scope stack holding, outermost first, the fallback provider
//!    ([`FlmFallbackProvider`], which resolves every name no declaration
//!    defines), techy's built-in package that handles `\begin` and `\end`, and the features'
//!    packages in registration order.
//! 3. **Builds the driver** ([`FlmDriver`]) and installs the [recursion
//!    guard](crate::lang::guard) of the policy that the inputs specify
//!    ([`LangInputs::guard`]).
//!
//! The fallback provider is at the bottom of the scope stack, so every defined
//! name takes precedence over it. Below every fallback a feature declares, the
//! provider holds the language's own two fallbacks: one for an unknown macro and one for
//! an unknown environment. Every name therefore resolves, in every parsing mode,
//! to a specification that minting built, and every node receives an element.
//!
//! # Errors are collected, never fatal
//!
//! [`build_language`] always returns a [`BuiltLang`]. Each problem in the
//! declarations (two features defining one name, an environment referring to a
//! scoped package nobody declared, a state that could not be derived) is
//! collected into [`BuiltLang::errors`] as a [`LangBuildError`], and the
//! language is built from the declarations that have no problem. The caller
//! decides whether to proceed; [`BuiltLang::is_sound`] tells whether there was
//! any problem.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use techy::core::specs::{ScopeOp, SpecsProvider};
use techy::core::{DeriveError, Language, ParsingState, ParsingStateDelta};
use techy::error::Recovery;

use crate::model::names::{FeatureName, Name};
use crate::lang::driver::FlmDriver;
use crate::lang::ext::FlmStateExt;
use crate::lang::flm::Flm;
use crate::lang::input::{InputResolver, Placements};
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::{
    mint_packages, ElementDef, FallbackDef, FlmFallbackProvider, MintError, OverrideDef,
    ScopedPackageDef, Symbol,
};
use crate::lang::rules::FlmParsing;
use crate::lang::structural::StructuralElements;

/// Everything [`build_language`] takes: the declarations of every feature, and
/// the settings that apply to every parse.
///
/// The registry fills this in. A test or a program that builds a language
/// without a registry starts from [`LangInputs::new`] and adds declarations to
/// the vectors.
pub struct LangInputs {
    /// The features' element declarations: each element with the syntax it is
    /// written in, in registration order. Where two features define the same
    /// name, the later feature's package takes precedence; two definitions in
    /// one parsing mode are also reported as a [`LangBuildError::Mint`] unless
    /// the later feature is explicitly after the other
    /// ([`explicitly_before`](Self::explicitly_before)) or its override
    /// declares the redefinition ([`overrides`](Self::overrides)).
    pub elements: Vec<ElementDef>,
    /// The features' fallbacks: policies for callables that no declaration
    /// defines, each limited to the parsing modes it applies in.
    pub fallbacks: Vec<FallbackDef>,
    /// The [scoped packages](crate::lang::mint::ScopedPackageDef): sets of
    /// element declarations that can be written only inside the environment
    /// bodies and the arguments whose declarations refer to them.
    pub scoped_packages: Vec<ScopedPackageDef>,
    /// The scoped packages that a feature expects another feature to declare,
    /// as pairs (the expecting feature, the package's name). Each pair is a
    /// [peer declaration](crate::core::registry::PeerName) of a scoped package.
    ///
    /// A body or an argument of the expecting feature's declarations may refer
    /// to such a package when no feature declares it: the reference then makes
    /// no command available, and it is not a
    /// [`MintError::UnknownScopedPackage`]. When some feature does declare the
    /// package, the pair has no effect. A registry fills this from its
    /// features' peer declarations and reports each one that no feature
    /// declares.
    pub peer_scoped_packages: Vec<(FeatureName, Name)>,
    /// The explicit redefinitions of a name another feature defines. Without
    /// one, two features defining the same name in one parsing mode is an
    /// error that identifies both features, unless
    /// [`explicitly_before`](Self::explicitly_before) places the feature
    /// registered last after the others. Only the override of the feature
    /// registered last among those that define the name in a mode is
    /// consulted, in that mode.
    pub overrides: Vec<OverrideDef>,
    /// The pairs `(a, b)` of features such that the feature order places `a`
    /// explicitly before `b` ([`crate::core::registry::order`]). Where features
    /// define one name in one parsing mode, the definition of the feature
    /// registered last applies there without an error when every other of
    /// those features is explicitly before it
    /// ([`crate::lang::mint::packages`]). The registry fills this from the
    /// chains of its feature order; it is empty by default.
    pub explicitly_before: BTreeSet<(FeatureName, FeatureName)>,
    /// The document's parsing settings: comment recognition, the comment
    /// prefix, and whether `$` delimits mathematics.
    pub parsing: FlmParsing,
    /// The structural elements: the registry's set, with the aspects the
    /// features attached to them.
    pub structural: Arc<StructuralElements>,
    /// The recursion-guard policy of every parse: the stack budget for normal
    /// use, a limit on open descents for tests ([`GuardPolicy`]).
    pub guard: GuardPolicy,
    /// How many diagnostics a parse keeps; `None` keeps techy's own default. One
    /// parse may set another cap through
    /// [`ParseOptions`](crate::lang::ParseOptions).
    pub diagnostics_limit: Option<usize>,
    /// The input resolver: the object, supplied by the embedding program, that
    /// finds and reads the file an `\input` command refers to
    /// ([`crate::lang::input`]). The library performs no input or output of its
    /// own; with `None`, `\input` includes no file and is reported.
    pub input_resolver: Option<Arc<dyn InputResolver>>,
}

impl LangInputs {
    /// Returns inputs with nothing declared: the default parsing settings, the
    /// default structural elements, the stack-budget guard, no cap on
    /// diagnostics of their own, and no input resolver.
    ///
    /// A language built from these recognizes FLM's own structure (text,
    /// comments, groups, mathematics) and no command of any feature.
    pub fn new() -> LangInputs {
        LangInputs {
            elements: Vec::new(),
            fallbacks: Vec::new(),
            scoped_packages: Vec::new(),
            peer_scoped_packages: Vec::new(),
            overrides: Vec::new(),
            explicitly_before: BTreeSet::new(),
            parsing: FlmParsing::default(),
            structural: Arc::new(StructuralElements::default()),
            guard: GuardPolicy::StackBudget,
            diagnostics_limit: None,
            input_resolver: None,
        }
    }
}

impl Default for LangInputs {
    fn default() -> LangInputs {
        LangInputs::new()
    }
}

impl fmt::Debug for LangInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LangInputs")
            .field("elements", &self.elements.len())
            .field("fallbacks", &self.fallbacks.len())
            .field("scoped_packages", &self.scoped_packages.len())
            .field("peer_scoped_packages", &self.peer_scoped_packages)
            .field("overrides", &self.overrides)
            .field("explicitly_before", &self.explicitly_before)
            .field("parsing", &self.parsing)
            .field("guard", &self.guard)
            .field("diagnostics_limit", &self.diagnostics_limit)
            .finish_non_exhaustive()
    }
}

/// A problem the language build found in its inputs. Problems are collected in
/// [`BuiltLang::errors`] and never stop the build: see the [module
/// documentation](self#errors-are-collected-never-fatal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LangBuildError {
    /// Minting could not build every declaration: for example, two features
    /// define one name without an override, or a body or an argument refers to
    /// a scoped package that nobody declared and its feature did not declare as
    /// a peer. [`MintError`] lists every case.
    Mint(MintError),
    /// The parsing state every parse starts from could not be derived exactly
    /// as the inputs asked. The language was built from the state techy recovered, without
    /// the scope changes that failed.
    State {
        /// techy's description of what failed.
        detail: String,
    },
}

impl fmt::Display for LangBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LangBuildError::Mint(error) => fmt::Display::fmt(error, f),
            LangBuildError::State { detail } => {
                write!(f, "the initial parsing state could not be derived: {detail}")
            }
        }
    }
}

impl std::error::Error for LangBuildError {}

/// A built language: what every parse of a document under one registry runs
/// under.
///
/// It holds the techy `Language` value, the parsing state every parse of a
/// document starts from, the providers and the structural elements that
/// state refers to, the table of every registered callable name, and the
/// problems the build collected. Pass it to
/// [`parse_fragment`](crate::lang::parse_fragment).
///
/// [`build_language`] creates it; a registry holds its own
/// ([`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang)). A
/// `BuiltLang` never changes after it is built.
pub struct BuiltLang {
    language: Language<Flm>,
    initial_state: Arc<ParsingState<Flm>>,
    providers: Vec<Arc<dyn SpecsProvider<Flm>>>,
    fallback: Arc<FlmFallbackProvider>,
    structural: Arc<StructuralElements>,
    symbols: BTreeMap<Symbol, FeatureName>,
    errors: Vec<LangBuildError>,
    parsing: FlmParsing,
    guard: GuardPolicy,
    driver: FlmDriver,
}

impl BuiltLang {
    /// Returns the techy language value: the driver, the initial state and the
    /// recursion guard, ready to parse.
    pub fn language(&self) -> &Language<Flm> {
        &self.language
    }

    /// Returns the parsing state every parse of a document starts from, which
    /// [`build_language`] derived from the language's initial parsing state
    /// (techy's `ParsingState::lang_initial`). To parse under other conditions, derive a state from it with
    /// [`BuiltLang::derive_state`].
    pub fn initial_state(&self) -> &Arc<ParsingState<Flm>> {
        &self.initial_state
    }

    /// Returns every provider of specifications that minting built, in one
    /// list: the fallback provider; the package holding the fallback
    /// specifications, which the fallback specifications' provenance stamps
    /// (the record of the package that holds a specification, used to
    /// serialize it by identity; see [`crate::lang::serialize`]) refer to; the
    /// package holding the specifications of the field macros that tack-on
    /// arguments absorb
    /// ([`ArgStdKind::TackOn`](crate::lang::mint::ArgStdKind::TackOn)); the
    /// declaring features' packages (one per feature and per set of parsing
    /// modes its declarations are available in); and the scoped packages.
    ///
    /// A [reading session](crate::dump::session) needs this list to rebuild a
    /// serialized parse, in which a specification is referred to by identity and
    /// looked up in the package it came from. techy's built-in package that
    /// handles `\begin` and `\end` is not in the list; it is reachable through
    /// the initial state's scope stack.
    pub fn providers(&self) -> &[Arc<dyn SpecsProvider<Flm>>] {
        &self.providers
    }

    /// Returns the fallback provider: the provider at the bottom of the scope
    /// stack that resolves a callable no declaration defines, applying each
    /// feature's fallback in the parsing modes that fallback declares. The
    /// specifications it returns are held in its own package
    /// ([`FlmFallbackProvider::package`]).
    pub fn fallback(&self) -> &Arc<FlmFallbackProvider> {
        &self.fallback
    }

    /// Returns the structural elements of this language: the elements of every
    /// node shape no feature declares. The initial state's extension holds the
    /// same set.
    pub fn structural(&self) -> &Arc<StructuralElements> {
        &self.structural
    }

    /// Returns every callable name the features' packages register, with the
    /// feature whose definition applies: the last registered, since a later
    /// feature's package takes precedence over an earlier one's. A name that two
    /// features define in parsing modes that do not overlap is listed once,
    /// also under the last registered.
    pub fn symbols(&self) -> &BTreeMap<Symbol, FeatureName> {
        &self.symbols
    }

    /// Returns the problems the build found; empty for a language without
    /// problems. See [`LangBuildError`].
    pub fn errors(&self) -> &[LangBuildError] {
        &self.errors
    }

    /// Returns whether the build found no problem at all
    /// ([`BuiltLang::errors`] is empty).
    pub fn is_sound(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the parsing settings the language was built with
    /// ([`LangInputs::parsing`]).
    pub fn parsing(&self) -> &FlmParsing {
        &self.parsing
    }

    /// Returns the recursion-guard policy every parse under this language runs
    /// with.
    pub fn guard(&self) -> GuardPolicy {
        self.guard
    }

    /// Returns the parse driver: the behavior and the settings of a running
    /// parse. [`BuiltLang::language_for_parse`] builds its language from a copy
    /// of it.
    pub fn driver(&self) -> &FlmDriver {
        &self.driver
    }

    /// Returns a parsing state derived from the initial one by applying
    /// `delta`.
    ///
    /// Use it to parse under other conditions than a document's while keeping
    /// what the build installed (the features' packages, the structural
    /// elements). The library's own use is the parse of a [user
    /// definition](crate::lang::rewrites::userdefs::Definition)'s template, which
    /// runs with the [`Template`](crate::lang::RestrictedInterior::Template)
    /// restricted-interior policy and the package of placeholders added. Pass
    /// the result to
    /// [`parse_fragment_with_state`](crate::lang::parse_fragment_with_state).
    ///
    /// # Errors
    ///
    /// techy's `DeriveError` when `delta` cannot be applied: a scope change the
    /// stack refuses, or an event that can only be interpreted during a parse.
    /// The error carries the state techy recovered.
    #[allow(clippy::result_large_err)]
    pub fn derive_state(
        &self,
        delta: &ParsingStateDelta<Flm>,
    ) -> Result<Arc<ParsingState<Flm>>, DeriveError<Flm>> {
        self.initial_state.derived(delta).map(Arc::new)
    }

    /// Returns a language with the same initial state and guard that keeps at
    /// most `diagnostics_limit` diagnostics per parse; `None` restores techy's
    /// own default. This is [`language_for_parse`](Self::language_for_parse)
    /// with this language's input resolver. Cheap: the state is shared and only
    /// the driver is copied.
    pub fn language_with(&self, diagnostics_limit: Option<usize>) -> Language<Flm> {
        self.language_for_parse(diagnostics_limit, None)
    }

    /// Returns a language with the same initial state and guard that keeps at
    /// most `diagnostics_limit` diagnostics per parse (`None` restores techy's
    /// own default) and reads included files with `input_resolver` when one is
    /// given (with this language's input resolver otherwise). A parse with
    /// options of its own ([`ParseOptions`](crate::lang::ParseOptions)) runs
    /// with such a language. Cheap: the state is shared and only the driver is
    /// copied.
    pub fn language_for_parse(
        &self,
        diagnostics_limit: Option<usize>,
        input_resolver: Option<Arc<dyn InputResolver>>,
    ) -> Language<Flm> {
        self.language_recording(diagnostics_limit, input_resolver, None)
    }

    /// Builds every per-parse language:
    /// [`language_for_parse`](Self::language_for_parse), with the driver also
    /// carrying `placements` when given (the record of heading inputs that the
    /// assembly of a document reads back, [`crate::model::assemble`]).
    pub(crate) fn language_recording(
        &self,
        diagnostics_limit: Option<usize>,
        input_resolver: Option<Arc<dyn InputResolver>>,
        placements: Option<Arc<Placements>>,
    ) -> Language<Flm> {
        let mut driver = self.driver.clone().with_diagnostics_limit(diagnostics_limit);
        if let Some(resolver) = input_resolver {
            driver = driver.with_input_resolver(resolver);
        }
        if let Some(placements) = placements {
            driver = driver.with_placements(placements);
        }
        guard::language(Language::new(driver, Arc::clone(&self.initial_state)), self.guard)
    }
}

impl fmt::Debug for BuiltLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuiltLang")
            .field("providers", &self.providers.iter().map(|p| p.name()).collect::<Vec<_>>())
            .field("symbols", &self.symbols.len())
            .field("errors", &self.errors)
            .field("parsing", &self.parsing)
            .field("guard", &self.guard)
            .finish()
    }
}

/// Builds the language from the declarations and settings in `inputs`.
///
/// The [module documentation](self) describes the three steps of the build and
/// how problems are collected. A [`BuiltLang`] is always returned; check
/// [`BuiltLang::is_sound`] before relying on it, and read the problems in
/// [`BuiltLang::errors`]. A registry calls this function when it is built; most
/// callers take the result from
/// [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang).
///
/// # Examples
///
/// ```
/// use flm_core::lang::{build_language, LangInputs};
///
/// // Nothing declared: FLM's own structure and no feature.
/// let lang = build_language(LangInputs::new());
/// assert!(lang.is_sound());
/// assert!(lang.symbols().is_empty());
/// // The fallback provider is always installed, so an unknown name still resolves.
/// assert!(lang.providers().iter().any(|p| p.name() == "_fallback.gate"));
/// ```
pub fn build_language(inputs: LangInputs) -> BuiltLang {
    let LangInputs {
        elements,
        fallbacks,
        scoped_packages,
        peer_scoped_packages,
        overrides,
        explicitly_before,
        parsing,
        structural,
        guard: policy,
        diagnostics_limit,
        input_resolver,
    } = inputs;

    let minted = mint_packages(
        &elements,
        &fallbacks,
        &scoped_packages,
        &overrides,
        &explicitly_before,
        &peer_scoped_packages,
        &structural,
    );
    let mut errors: Vec<LangBuildError> =
        minted.errors.into_iter().map(LangBuildError::Mint).collect();
    let fallback = minted.fallback;

    // FLM's seed cannot fail (its data are in-crate constants); the `expect`
    // states that invariant.
    let seed = ParsingState::<Flm>::lang_initial()
        .expect("the initial parsing state is built from constants and cannot fail");

    // The scope stack, outermost first: the fallback gate at the bottom (so every
    // defined name shadows it), the seed's builtin dispatch package, then the
    // features' packages in registration order.
    let mut stack: Vec<Arc<dyn SpecsProvider<Flm>>> = Vec::new();
    let mut providers: Vec<Arc<dyn SpecsProvider<Flm>>> = Vec::new();
    let gate: Arc<dyn SpecsProvider<Flm>> = fallback.clone();
    stack.push(Arc::clone(&gate));
    providers.push(gate);
    let fallback_package: Arc<dyn SpecsProvider<Flm>> = fallback.package().clone();
    providers.push(fallback_package);
    let fields_package: Arc<dyn SpecsProvider<Flm>> = minted.fields.clone();
    providers.push(fields_package);
    stack.extend(seed.scopes().providers().iter().cloned());
    for package in &minted.packages {
        let provider: Arc<dyn SpecsProvider<Flm>> = package.clone();
        stack.push(Arc::clone(&provider));
        providers.push(provider);
    }
    for package in minted.scoped.values() {
        let provider: Arc<dyn SpecsProvider<Flm>> = package.clone();
        providers.push(provider);
    }

    let delta = ParsingStateDelta::new()
        .rules(parsing.token_rules_overrides(seed.rules()))
        .ext(FlmStateExt::new(Arc::clone(&structural)).with_parsing(parsing.clone()))
        .scope_op(ScopeOp::ReplaceStack(stack));
    let initial_state = match seed.derived(&delta) {
        Ok(state) => Arc::new(state),
        Err(error) => {
            errors.push(LangBuildError::State { detail: error.to_string() });
            Arc::new(error.recovered)
        }
    };

    let mut driver = FlmDriver::new(Recovery::Tolerant, diagnostics_limit);
    if let Some(resolver) = input_resolver {
        driver = driver.with_input_resolver(resolver);
    }
    let language =
        guard::language(Language::new(driver.clone(), Arc::clone(&initial_state)), policy);

    BuiltLang {
        language,
        initial_state,
        providers,
        fallback,
        structural,
        symbols: minted.symbols,
        errors,
        parsing,
        guard: policy,
        driver,
    }
}
