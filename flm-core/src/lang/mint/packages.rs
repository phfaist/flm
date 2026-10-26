//! [`mint_packages`], which builds the techy packages and the fallback provider
//! from the features' declarations, and reports as [`MintError`] values the
//! declarations that do not fit.
//!
//! [`mint_packages`] takes the declarations of every feature and returns the
//! [`MintedProviders`] that the language build
//! ([`build_language`](crate::lang::build_language)) installs. Every
//! specification a parse can resolve comes from here. The [module
//! documentation](super) explains minting and the terms techy package,
//! specification and scope stack.
//!
//! # What minting builds
//!
//! - **The features' techy packages**, holding every element declaration
//!   under every name it is written under. A feature has one techy package
//!   per distinct set of parsing modes its declarations list. The first is
//!   named after the feature, and is the only one when all of the feature's
//!   declarations list the same modes; each further one is named
//!   `<feature>#<n>`, with `n` counting from 2. The language build puts the
//!   packages on the scope stack in registration order (each feature at the
//!   place of its first element declaration, so that the declarations a
//!   feature makes from a finalizer join its others), and a feature's own
//!   packages in the order their mode sets first appear, so a later
//!   feature's package shadows an earlier one's.
//!
//!   Because a techy package holds one entry per callable kind and name,
//!   these several packages let one feature declare one name twice in
//!   parsing modes that do not overlap (a text-only `\vec` and a
//!   mathematics-only `\vec`). Declaring one name twice in modes that
//!   overlap is reported ([`MintError::Redeclared`]).
//! - **The scoped packages**, one techy package per [`ScopedPackageDef`].
//!   They are not on the scope stack: an environment body or an argument that
//!   refers to one adds it to the scope stack while that body or argument is
//!   parsed. Each is built after the scoped packages its own declarations
//!   refer to (in an environment's body or in any argument), so a reference
//!   always finds a package that already exists.
//! - **The fallback provider** ([`FlmFallbackProvider`]), at the bottom of
//!   the scope stack, which resolves a command that no declaration defines.
//! - **The fields package**, holding the specifications of tack-on fields.
//!
//! Every specification is one of the types of [`super::specs`] and carries a
//! provenance stamp (techy's
//! [`SpecProvenance`](techy::core::specs::SpecProvenance)): the record,
//! issued by the techy package the specification is inserted into, of that
//! package and of the key the specification is held under. A serialized tree
//! ([`crate::lang::serialize`]) refers to a specification through its stamp.
//!
//! # Two features that define one name
//!
//! Minting detects clashes on the built techy packages themselves, asking
//! each for the names it registers, per callable kind and per parsing mode.
//! Each parsing mode is decided on its own. Where two or more features define
//! a name in one mode, registration order decides which definition applies in
//! that mode: the later feature's techy package is innermost on the scope
//! stack and shadows the others. The registration order is the final order of
//! the features, which the chains of the feature order decide
//! ([`crate::core::registry::order`]).
//!
//! Such a collision is allowed in a mode when the feature registered last
//! among the features that define the name in that mode is explicitly after
//! every other of them in the feature order (`a < b` lets `b` define a name
//! `a` defines), or when that feature declared an override ([`OverrideDef`]):
//! the collision is then intended and is not reported. Otherwise it is one
//! [`MintError::Clash`] for that mode, which lists every feature that defines
//! the name in it. Only the override of the definition that applies is
//! consulted: an override that another of those features declared has no
//! effect.
//!
//! Definitions in different modes never clash: a `\vec` for text and a `\vec`
//! for mathematics are two different commands. A feature that defines a name
//! in text mode only and another that defines it in math mode only are
//! therefore never in a collision, even where a third feature defines the name
//! in both modes; that third feature then meets each of them in one mode.
//!
//! Scoped packages are not checked for clashes, neither against one another
//! nor against the features' own packages, because a scoped package is
//! available only inside the bodies and arguments that refer to it. Two
//! scoped packages that declare one name (two features each giving `\\` a
//! meaning inside its own environment, in text mode) are therefore available
//! in separate places, unless one is used inside the other; the inner one then
//! shadows the outer one while its body or argument is parsed, as any techy
//! package added later does. Only two scoped packages with the same package
//! name clash ([`ClashKind::ScopedPackage`]), since a body refers to a
//! package by its name.
//!
//! # The fallback package and the fields package
//!
//! Two techy packages are never on the scope stack.
//!
//! **The fallback package**, named [`FALLBACK_PACKAGE_NAME`], holds the
//! fallback specifications under keys built from the owner, the callable kind
//! and the position of each fallback. It exists so that those specifications
//! carry provenance stamps like every other and can be serialized and read
//! back. The scope stack holds the fallback provider instead, which returns a
//! specification from this package in the declared modes only. The fallback
//! provider resolves *any* name and so cannot list its entries; a [reading
//! session](crate::dump::session) is given the package beside it, and
//! [`BuiltLang::providers`](crate::lang::BuiltLang::providers) lists it.
//!
//! After every fallback a feature declares, the fallback provider consults
//! the language's own two, each applying in every mode: a macro that no declaration
//! defines resolves to a specification carrying `flm.unknown.macro` and is
//! reported, and an environment likewise to one carrying `flm.unknown.environment`.
//! A feature's fallback for a mode shadows the language's in that mode. This is how
//! mathematics accepts any command without a report when a mathematics feature is
//! registered, while text mode still reports unknown commands.
//!
//! **The fields package**, named [`FIELDS_PACKAGE_NAME`], holds the
//! specifications of [tack-on fields](super::TackOnField). A field's node
//! records such a specification, but no name lookup ever reaches it: the
//! parser that absorbs the fields holds it directly. The package exists so
//! that these specifications too carry provenance stamps and can be
//! serialized. Their keys are built from the declaration alone, so adding an
//! unrelated declaration elsewhere does not change the key a serialized tree
//! refers to.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{ArgumentParser, TackOnFieldsArgumentParser};
use techy::core::specs::{
    ArgumentSpec, CallableQuery, CallableSpec, Package, ProviderError, SpecsProvider,
};
use techy::core::{ClosedVocabulary, ParsingState, ParsingStateDelta};
use techy::latexlike::{Event, MacroSpec, Mode, SpecialsSpec};

use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, FeatureName, Name};
use crate::lang::flm::{Flm, FlmCallableType};
use crate::lang::mint::decl::{
    ArgDef, ArgLayout, BodyDef, BodyLayout, CallableKind, ElementDef, FallbackDef,
    FallbackKind, OverrideDef, ScopedPackageDef, SyntaxDef, TackOnField,
};
use crate::lang::mint::opaque::OpaqueWrapperReader;
use crate::lang::mint::specs::{
    FlmCallable, FlmEnvironment, FlmUnknownEnvironmentBehavior, FlmUnknownMacroSpec,
};
use crate::lang::input::InputSpec;
use crate::lang::structural::StructuralElements;

/// One registered command name: its callable kind and the name itself (the
/// trigger, for a special character sequence).
///
/// Its `Display` text, such as ``macro `vec` ``, appears in error messages.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct Symbol {
    /// The callable kind of the name.
    pub kind: CallableKind,
    /// The name without its escape character, or the trigger.
    pub name: String,
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} `{}`", self.kind, self.name)
    }
}

/// What was declared more than once, in a [`MintError::Clash`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashKind {
    /// Two or more features define one command name in parsing modes that
    /// overlap.
    Symbol,
    /// Two fallbacks for one callable kind, declared by one feature or by
    /// two, apply in parsing modes that overlap.
    Fallback,
    /// A scoped package name is declared twice, by one feature or by two.
    ScopedPackage,
}

/// A problem with the declarations that minting found.
///
/// [`mint_packages`] collects every problem in
/// [`MintedProviders::errors`] and still builds everything that is sound;
/// each variant says what was found and, where a choice had to be made, which
/// declaration applies. The language build reports each one as a
/// [`LangBuildError::Mint`](crate::lang::LangBuildError::Mint), and the
/// registry build refuses a language with any such error: it fails in its
/// language phase
/// ([`BuildPhase::Language`](crate::core::registry::BuildPhase::Language)).
/// The `userdefs` feature, which mints a language for the templates of [user
/// definitions](crate::lang::rewrites::userdefs::Definition), reports these
/// errors as diagnostics of those user definitions.
///
/// The `Display` text states the problem for the author of the feature or of
/// the user definition; it identifies, where the problem concerns them, the
/// declaring feature and the element by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintError {
    /// Two or more features declare the same thing where it matters. For a
    /// command name: two or more features define it in one parsing mode, and
    /// the feature registered last among them in that mode is neither placed
    /// by the feature order explicitly after every other of them nor declares
    /// an override. There is one error per name and parsing mode.
    Clash {
        /// What was declared twice.
        kind: ClashKind,
        /// The name concerned: the command name; for a fallback, the name of
        /// the callable kind; for a scoped package, the package's name.
        symbol: Symbol,
        /// The parsing modes in which the declarations overlap, in the modes'
        /// fixed order: for a command name, the one mode concerned. Empty for a
        /// scoped package, which is not tied to parsing modes.
        modes: Vec<Mode>,
        /// Every feature whose declaration is available in one of `modes`, in
        /// registration order.
        owners: Vec<FeatureName>,
        /// The first of `owners`.
        first: FeatureName,
        /// The feature whose declaration applies despite the clash: for a
        /// command name the last of `owners`, since a later feature's package
        /// shadows an earlier one's; for a fallback or a scoped package the
        /// first, since a later overlapping one is not installed.
        winner: FeatureName,
    },
    /// An environment body or an argument refers to a scoped package that no
    /// feature declares and that the feature declaring the element did not
    /// declare as a peer. (A package declared as a peer that no feature
    /// declares makes nothing available and is reported by the registry build
    /// as a Note, not here.)
    UnknownScopedPackage {
        /// The element whose declaration refers to the package.
        element: ElementName,
        /// The feature declaring the element.
        owner: FeatureName,
        /// `None` when the environment's body refers to the package; the
        /// argument's position, counting from zero, when an argument does, in
        /// the argument list that declares it: the element's own, or that of
        /// the tack-on field given by `field`.
        argument: Option<usize>,
        /// The name of the [tack-on field](TackOnField) whose own argument
        /// list holds the argument; `None` when the argument is the element's
        /// own.
        field: Option<String>,
        /// The package name that was not found.
        package: Name,
    },
    /// Scoped packages refer to each other in a cycle, each through one of its
    /// declarations (an environment's body or an argument), so there is no
    /// order in which each is built after the ones it refers to. Minting
    /// breaks the cycle at the reference that closes it.
    ScopedPackageCycle {
        /// The packages of the cycle, each referring to the next, the last to
        /// the first.
        packages: Vec<Name>,
    },
    /// A special character sequence declaration lists command names. It may
    /// list none: its trigger is how it is written.
    SpecialsWithNames {
        /// The declaring feature.
        owner: FeatureName,
        /// The trigger of the declaration.
        trigger: String,
    },
    /// A declaration with no syntax ([`SyntaxDef::Synthesized`]) lists command
    /// names. It may list none, since nothing in the source writes it.
    SynthesizedWithNames {
        /// The declaring feature.
        owner: FeatureName,
        /// The element concerned.
        element: ElementName,
    },
    /// A file-inclusion macro ([`SyntaxDef::Inclusion`]) declares no argument
    /// named [`REFERENCE_ARGUMENT`](crate::lang::input::REFERENCE_ARGUMENT), so
    /// it has no argument to read the file's reference from.
    NoReferenceArgument {
        /// The declaring feature.
        owner: FeatureName,
        /// The element concerned.
        element: ElementName,
        /// A description of the declared arguments, such as "it declares no
        /// argument".
        detail: String,
    },
    /// One feature declares one command name twice in parsing modes that
    /// overlap, so in the overlapping modes only one of the two declarations
    /// could apply. Two declarations of one name whose modes do *not* overlap
    /// are sound and are built into two of the feature's techy packages.
    Redeclared {
        /// The name concerned.
        symbol: Symbol,
        /// The feature declaring it twice.
        owner: FeatureName,
        /// The parsing modes both declarations are available in, in the modes'
        /// fixed order.
        modes: Vec<Mode>,
    },
    /// A macro or environment declaration lists no name, so nothing can be
    /// registered for it.
    NoNames {
        /// The declaring feature.
        owner: FeatureName,
        /// The element concerned.
        element: ElementName,
    },
    /// An argument declared with an opaque wrapper
    /// ([`ArgDef::opaque_wrapper`]) declares a [layout](ArgLayout) other than
    /// [`ArgLayout::Inline`]. The [block-structure
    /// pass](crate::lang::rewrites::blocks) carries the content of an opaque
    /// wrapper over as written, so the declaration could never take effect.
    /// The argument is built with the inline layout.
    LayoutOnOpaqueWrappedArgument {
        /// The declaring feature.
        owner: FeatureName,
        /// The element whose declaration holds the argument.
        callable: ElementName,
        /// The argument's position, counting from zero, in the argument list
        /// that declares it: the element's own, or that of the tack-on field
        /// given by `field`.
        index: usize,
        /// The name of the [tack-on field](TackOnField) whose own argument
        /// list holds the argument; `None` when the argument is the element's
        /// own.
        field: Option<String>,
    },
    /// An argument read in mathematics mode ([`ArgDef::mode`]) declares the
    /// layout [`ArgLayout::Blocks`]. The [block-structure
    /// pass](crate::lang::rewrites::blocks) carries mathematics over
    /// untouched, so the declaration could never take effect. The argument is
    /// built with the inline layout.
    BlocksOnMathArgument {
        /// The declaring feature.
        owner: FeatureName,
        /// The element whose declaration holds the argument.
        callable: ElementName,
        /// The argument's position, counting from zero, in the argument list
        /// that declares it: the element's own, or that of the tack-on field
        /// given by `field`.
        index: usize,
        /// The name of the [tack-on field](TackOnField) whose own argument
        /// list holds the argument; `None` when the argument is the element's
        /// own.
        field: Option<String>,
    },
    /// An environment's body declared with an opaque wrapper
    /// ([`BodyDef::opaque_wrapper`]) declares a [layout](BodyDef::layout)
    /// other than [`FromRole`](super::BodyLayout::FromRole). The
    /// [block-structure pass](crate::lang::rewrites::blocks) carries the
    /// content of an opaque wrapper over as written, so the declaration could
    /// never take effect.
    LayoutOnOpaqueWrappedBody {
        /// The declaring feature.
        owner: FeatureName,
        /// The environment's element.
        element: ElementName,
    },
}

/// Returns the words that identify the tack-on field an argument belongs to,
/// for a message: empty for an element's own argument.
fn of_field(field: &Option<String>) -> String {
    field.as_ref().map(|name| format!(" of the field `{name}`")).unwrap_or_default()
}

/// Returns feature names written as `` feature `a` `` or
/// `` features `a`, `b` and `c` ``, for a message.
fn owner_list(owners: &[FeatureName]) -> String {
    match owners {
        [] => String::from("no feature"),
        [one] => format!("feature `{one}`"),
        [head @ .., last] => {
            let head: Vec<String> = head.iter().map(|owner| format!("`{owner}`")).collect();
            format!("features {} and `{last}`", head.join(", "))
        }
    }
}

/// Returns parsing modes written as `mode Text` or `modes Text and Math`, for
/// a message (an empty list, which no declaration carries, reads `no mode`).
fn mode_list(modes: &[Mode]) -> String {
    match modes {
        [] => String::from("no mode"),
        [one] => format!("mode {one:?}"),
        [head @ .., last] => {
            let head: Vec<String> = head.iter().map(|mode| format!("{mode:?}")).collect();
            format!("modes {} and {last:?}", head.join(", "))
        }
    }
}

/// Returns quoted names written as `` `a` ``, `` `a` and `b` `` or
/// `` `a`, `b`, and `c` ``, for a message.
fn series(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [head @ .., last] => format!("{}, and {last}", head.join(", ")),
    }
}

/// Returns parsing modes written as `in math mode` or `in text and math
/// modes`, for a message.
fn in_modes(modes: &[Mode]) -> String {
    let words: Vec<String> = modes.iter().map(|mode| format!("{mode:?}").to_lowercase()).collect();
    match words.as_slice() {
        [] => String::from("in no mode"),
        [one] => format!("in {one} mode"),
        [head @ .., last] => format!("in {} and {last} modes", head.join(", ")),
    }
}

impl fmt::Display for MintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MintError::Clash { kind: ClashKind::Symbol, symbol, modes, owners, winner, .. } => {
                // The others from the closest to the winner on the scope stack;
                // the chain in the order of the stack, which respects every
                // chain written, so it never makes a cycle.
                let construct = match symbol.kind {
                    CallableKind::Macro => format!("`\\{}`", symbol.name),
                    _ => format!("`{}`", symbol.name),
                };
                let others: Vec<String> = owners
                    .iter()
                    .filter(|owner| *owner != winner)
                    .rev()
                    .map(|owner| format!("`{owner}`"))
                    .collect();
                let chain: Vec<&str> = owners.iter().map(|owner| owner.as_str()).collect();
                write!(
                    f,
                    "Construct {construct} from `{winner}` clashes with {construct} defined in {} \
                     {}; either set `flm: feature_order: ['{}']` in your front matter to choose \
                     `{winner}`'s definition, or have `{winner}` declare {construct} as an explicit \
                     override.",
                    series(&others),
                    in_modes(modes),
                    chain.join(" < ")
                )
            }
            MintError::Clash { kind: ClashKind::Fallback, symbol, modes, owners, .. } => {
                write!(
                    f,
                    "the fallbacks for unknown {}s declared by {} overlap in {}; only one \
                     fallback for unknown {}s may apply in a parsing mode",
                    symbol.name,
                    owner_list(owners),
                    mode_list(modes),
                    symbol.name
                )
            }
            MintError::Clash { kind: ClashKind::ScopedPackage, symbol, owners, .. } => write!(
                f,
                "the scoped package name `{}` is declared more than once (by {}); a \
                 scoped package name may be declared only once",
                symbol.name,
                owner_list(owners)
            ),
            MintError::Redeclared { symbol, owner, modes } => write!(
                f,
                "feature `{owner}` declares {symbol} twice in {}; a feature may declare a \
                 name only once per parsing mode",
                mode_list(modes)
            ),
            MintError::UnknownScopedPackage { element, owner, argument: None, package, .. } => write!(
                f,
                "the body of `{element}` (an element of the feature `{owner}`) refers to \
                 the scoped package `{package}`, which no registered feature declares; a \
                 feature that refers to another feature's scoped package must declare it \
                 with `FeatureBuilder::peer_scoped_package`"
            ),
            MintError::UnknownScopedPackage {
                element,
                owner,
                argument: Some(index),
                field,
                package,
            } => {
                write!(
                    f,
                    "argument #{}{} of `{element}` (an element of the feature `{owner}`) \
                     refers to the scoped package `{package}`, which no registered feature \
                     declares; a feature that refers to another feature's scoped package \
                     must declare it with `FeatureBuilder::peer_scoped_package`",
                    index + 1,
                    of_field(field)
                )
            }
            MintError::ScopedPackageCycle { packages } => {
                let names: Vec<String> = packages.iter().map(|name| format!("`{name}`")).collect();
                write!(
                    f,
                    "scoped packages must not refer to each other in a cycle; the cycle is \
                     {}, the last referring back to the first",
                    names.join(" → ")
                )
            }
            MintError::SpecialsWithNames { owner, trigger } => write!(
                f,
                "feature `{owner}`: the special character sequence `{trigger}` must not \
                 declare command names; it is written as `{trigger}`"
            ),
            MintError::SynthesizedWithNames { owner, element } => write!(
                f,
                "feature `{owner}`: the element `{element}`, declared as never written in a \
                 document (`SyntaxDef::Synthesized`), must not declare command names"
            ),
            MintError::NoReferenceArgument { owner, element, detail } => write!(
                f,
                "feature `{owner}`: the file-inclusion macro of the element `{element}` must \
                 declare an argument named `reference` ({detail})"
            ),
            MintError::NoNames { owner, element } => {
                write!(
                    f,
                    "feature `{owner}`: the macro or environment declaration of the element \
                     `{element}` must list at least one name"
                )
            }
            MintError::LayoutOnOpaqueWrappedArgument { owner, callable, index, field } => write!(
                f,
                "feature `{owner}`: argument #{}{} of `{callable}` must be laid out inline, \
                 because it is declared with an opaque wrapper",
                index + 1,
                of_field(field)
            ),
            MintError::BlocksOnMathArgument { owner, callable, index, field } => write!(
                f,
                "feature `{owner}`: argument #{}{} of `{callable}` must not be laid out as \
                 blocks, because it is read in mathematics mode",
                index + 1,
                of_field(field)
            ),
            MintError::LayoutOnOpaqueWrappedBody { owner, element } => write!(
                f,
                "feature `{owner}`: the body of `{element}` must not declare a layout, because \
                 it is declared with an opaque wrapper"
            ),
        }
    }
}

impl std::error::Error for MintError {}

// --- the fallback provider --------------------------------------------------------------

/// The name of the fallback provider ([`FlmFallbackProvider`]) as a
/// specification provider on the scope stack, `_fallback.gate`.
pub const FALLBACK_PROVIDER_NAME: &str = "_fallback.gate";

/// The name of the techy package holding the fallback specifications
/// ([`FlmFallbackProvider::package`]), `_fallback`.
pub const FALLBACK_PACKAGE_NAME: &str = "_fallback";

/// The name of the techy package holding the specifications of tack-on fields
/// ([`MintedProviders::fields`]), `_fields`; see the [module
/// documentation](self).
pub const FIELDS_PACKAGE_NAME: &str = "_fields";

/// The feature name recorded as the owner of the language's own declarations,
/// `flm`: the fallbacks for an unknown macro and an unknown environment. No
/// registered feature has this name; [`FlmFallbackProvider::entries`] lists
/// these two fallbacks under it.
pub const LANGUAGE_OWNER: &str = "flm";

/// One fallback the fallback provider holds: the feature that declared it, the modes it
/// applies in, the callable kind it applies to, and its key in the fallback
/// package.
struct FallbackEntry {
    owner: FeatureName,
    modes: Vec<Mode>,
    kind: FallbackKind,
    key: String,
}

/// A fallback while it is being collected, before the package is built.
struct PendingFallback {
    owner: FeatureName,
    modes: Vec<Mode>,
    kind: FallbackKind,
    element: Arc<ElementSpec>,
    /// Whether this is one of the language's own fallbacks, which report the
    /// unknown callable, rather than a feature's, which passes it silently.
    recovery: bool,
}

/// The fallback provider: the specification provider that resolves a macro or
/// an environment that no declaration defines, according to the fallbacks
/// ([`FallbackDef`]) declared for the current parsing mode.
///
/// techy's own fallback provider cannot be restricted to parsing modes, so
/// minting builds this provider instead. The language build puts it at the
/// bottom of the scope stack, so every defined name shadows it. It returns
/// specifications from the techy package it holds
/// ([`FlmFallbackProvider::package`]).
///
/// The fallbacks are consulted in declaration order, with the language's own
/// two last, so a feature's fallback for a mode applies in that mode. In a
/// mode no feature's fallback covers, the language's own fallbacks apply: the
/// command is reported and its node carries `flm.unknown.macro` or
/// `flm.unknown.environment`. Text mode therefore reports unknown commands,
/// while mathematics mode, with a mathematics feature registered, does not.
///
/// It resolves *any* name, so it cannot list its entries; the package
/// beside it is therefore listed separately in [`BuiltLang::providers`].
///
/// [`BuiltLang::providers`]: crate::lang::BuiltLang::providers
pub struct FlmFallbackProvider {
    name: Box<str>,
    package: Arc<Package<Flm>>,
    entries: Vec<FallbackEntry>,
}

impl FlmFallbackProvider {
    /// Builds the fallback package and the fallback provider from the
    /// collected fallbacks, keeping their order.
    fn build(pending: &[PendingFallback]) -> FlmFallbackProvider {
        let mut entries = Vec::with_capacity(pending.len());
        let package = Package::new_shared(FALLBACK_PACKAGE_NAME, |pkg| {
            for (index, fallback) in pending.iter().enumerate() {
                let key = format!("{}.{}.{index}", fallback.owner, fallback.kind);
                let callable_type = fallback.kind.callable_type();
                let stamp = pkg.provenance_for(callable_type, key.as_str());
                match fallback.kind {
                    FallbackKind::Macro => {
                        let mut callable = if fallback.recovery {
                            FlmCallable::new(
                                FlmUnknownMacroSpec::new(),
                                Arc::clone(&fallback.element),
                            )
                        } else {
                            FlmCallable::new(
                                MacroSpec::<Flm>::new(Vec::new()),
                                Arc::clone(&fallback.element),
                            )
                        };
                        if let Some(stamp) = stamp {
                            callable = callable.with_provenance(stamp);
                        }
                        pkg.insert(callable_type, key.as_str(), callable);
                    }
                    FallbackKind::Environment => {
                        // The language layer's own environment body is block
                        // content; a feature's silent passthrough follows the
                        // element's role (the default declaration).
                        let body = if fallback.recovery {
                            BodyDef::new().blocks().parser(FlmUnknownEnvironmentBehavior::new())
                        } else {
                            BodyDef::default()
                        };
                        let environment =
                            FlmEnvironment::new(Vec::new(), Arc::clone(&fallback.element), body);
                        let mut spec = environment.into_spec();
                        if let Some(stamp) = stamp {
                            spec = spec.with_provenance(stamp);
                        }
                        pkg.insert(callable_type, key.as_str(), spec);
                    }
                }
                entries.push(FallbackEntry {
                    owner: fallback.owner.clone(),
                    modes: fallback.modes.clone(),
                    kind: fallback.kind,
                    key,
                });
            }
        });
        FlmFallbackProvider { name: FALLBACK_PROVIDER_NAME.into(), package, entries }
    }

    /// Returns the techy package holding the fallback specifications, which
    /// their provenance stamps refer to. It is never on the scope stack.
    pub fn package(&self) -> &Arc<Package<Flm>> {
        &self.package
    }

    /// Returns the installed fallbacks as `(owner, modes, kind)`, in the order
    /// they are consulted: declaration order, with the language's own two last
    /// (owned by [`LANGUAGE_OWNER`]). A fallback reported as a
    /// [`MintError::Clash`] is not among them.
    pub fn entries(&self) -> impl Iterator<Item = (&FeatureName, &[Mode], FallbackKind)> + '_ {
        self.entries.iter().map(|entry| (&entry.owner, entry.modes.as_slice(), entry.kind))
    }

    /// Returns whether some fallback applies to `kind` in `mode`. It always
    /// returns `true`, since the language's own two fallbacks apply to both
    /// kinds in every mode.
    pub fn answers(&self, kind: FallbackKind, mode: Mode) -> bool {
        self.entries.iter().any(|entry| entry.kind == kind && entry.modes.contains(&mode))
    }

    /// Returns the specification for `kind` in `mode`: that of the first
    /// installed fallback that covers them.
    pub fn spec_for(
        &self,
        kind: FallbackKind,
        mode: Mode,
    ) -> Option<&Arc<dyn CallableSpec<Flm>>> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.kind == kind && entry.modes.contains(&mode))?;
        self.package.get(kind.callable_type(), &entry.key)
    }
}

impl fmt::Debug for FlmFallbackProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmFallbackProvider")
            .field("name", &self.name)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl SpecsProvider<Flm> for FlmFallbackProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn retrieve_spec(
        &self,
        query: &CallableQuery<'_, Flm>,
        state: &ParsingState<Flm>,
    ) -> Result<Option<Arc<dyn CallableSpec<Flm>>>, ProviderError> {
        let kind = match query.callable_type {
            FlmCallableType::Macro => FallbackKind::Macro,
            FlmCallableType::Environment => FallbackKind::Environment,
            _ => return Ok(None),
        };
        Ok(self.spec_for(kind, state.mode()).cloned())
    }
}

// --- argument minting ----------------------------------------------------------------

/// One declared argument list, built: the argument specifications, and the
/// layout of each argument in the same order, which the command's
/// specification records beside its element.
struct MintedArgs {
    specs: Vec<Arc<ArgumentSpec<Flm>>>,
    layouts: Vec<ArgLayout>,
}

/// The scoped packages a declaration may refer to: those already built, by
/// name; the names some feature declares, which tell a package caught in a
/// cycle (declared, and reported separately) from one that no feature
/// declares; and the names each feature declares as a peer, as (feature,
/// name), which that feature's declarations may refer to although no feature
/// declares them.
struct Scopes<'a> {
    minted: &'a BTreeMap<Name, Arc<Package<Flm>>>,
    declared: &'a BTreeSet<Name>,
    peers: &'a BTreeSet<(FeatureName, Name)>,
}

/// Builds one declared argument list: the element's own when `field` is
/// `None`, else that of the tack-on field of this name, which an error
/// identifies. `home` is the fields package that receives the specifications
/// of any tack-on fields, `def` the declaration they belong to, from which
/// their keys are built and which an error identifies, and `scopes` the
/// scoped packages an argument may refer to.
fn argument_specs_of(
    args: &[ArgDef],
    field: Option<&str>,
    home: &mut Package<Flm>,
    def: &ElementDef,
    scopes: &Scopes<'_>,
    errors: &mut Vec<MintError>,
) -> MintedArgs {
    let mut minted = MintedArgs { specs: Vec::new(), layouts: Vec::new() };
    for (index, arg) in args.iter().enumerate() {
        minted.specs.push(argument_spec_of(arg, index, field, home, def, scopes, errors));
        minted.layouts.push(declared_layout(arg, index, field, def, errors));
    }
    minted
}

/// Returns the layout `arg` is built with: its declared one, or inline when the
/// argument can never take the declared one, which is reported
/// ([`MintError::LayoutOnOpaqueWrappedArgument`],
/// [`MintError::BlocksOnMathArgument`]).
fn declared_layout(
    arg: &ArgDef,
    index: usize,
    field: Option<&str>,
    def: &ElementDef,
    errors: &mut Vec<MintError>,
) -> ArgLayout {
    let owner = def.owner.clone();
    let callable = def.element.name.clone();
    let field = field.map(str::to_string);
    let error = if arg.opaque_wrapper && arg.layout != ArgLayout::Inline {
        MintError::LayoutOnOpaqueWrappedArgument { owner, callable, index, field }
    } else if arg.layout == ArgLayout::Blocks && arg.mode == Some(Mode::Math) {
        MintError::BlocksOnMathArgument { owner, callable, index, field }
    } else {
        return arg.layout;
    };
    errors.push(error);
    ArgLayout::Inline
}

/// Returns the key a tack-on field's specification is held under in the
/// fields package, which is also how a serialized tree refers to it.
///
/// It is built from the declaration alone, as `owner.element.field`. If that
/// key is taken (one declaration with two tack-on arguments accepting the same
/// field name), the argument's position in the declaration is appended; if
/// that is taken too, a running number is appended. The key therefore stays
/// the same when unrelated declarations change, and since `.` is not a
/// character of command names, no name written in the source can reach these
/// keys.
fn field_key(
    home: &Package<Flm>,
    def: &ElementDef,
    field: &TackOnField,
    arg_index: usize,
) -> String {
    let free = |key: &str| home.get(FlmCallableType::Macro, key).is_none();
    let base = format!("{}.{}.{}", def.owner, def.element.name, field.name);
    if free(&base) {
        return base;
    }
    let indexed = format!("{base}.{arg_index}");
    if free(&indexed) {
        return indexed;
    }
    (1..)
        .map(|number| format!("{indexed}.{number}"))
        .find(|key| free(key))
        .expect("the running numbers are unbounded, so a free key exists")
}

/// Builds the specification of one tack-on field and inserts it into the
/// fields package under its [`field_key`], with that package's provenance
/// stamp. It returns the value the package holds, so a field node records the
/// same specification that a serialized tree finds again by its key.
fn field_spec(
    field: &TackOnField,
    home: &mut Package<Flm>,
    def: &ElementDef,
    arg_index: usize,
    scopes: &Scopes<'_>,
    errors: &mut Vec<MintError>,
) -> Arc<dyn CallableSpec<Flm>> {
    let args = argument_specs_of(&field.args, Some(&field.name), home, def, scopes, errors);
    let syntax = MacroSpec::<Flm>::new(args.specs);
    let key = field_key(home, def, field, arg_index);
    let mut callable =
        FlmCallable::new(syntax, Arc::clone(field.element())).with_arg_layouts(args.layouts);
    if let Some(stamp) = home.provenance_for(FlmCallableType::Macro, key.as_str()) {
        callable = callable.with_provenance(stamp);
    }
    home.insert(FlmCallableType::Macro, key.as_str(), callable);
    let spec = home
        .get(FlmCallableType::Macro, &key)
        .expect("the field's specification was just inserted");
    Arc::clone(spec)
}

/// Builds one declared argument, the `arg_index`-th of its declaration, as a
/// techy argument specification: its parser, and the change of parsing state
/// its declared mode and scoped packages ask for. `field`, `home`, `def` and
/// `scopes` are as for [`argument_specs_of`].
///
/// The parser is the declaration's own object ([`ArgDef::kind`]), except for
/// a tack-on argument: its fields are declarations whose specifications are
/// built here into `home`, so the reader that absorbs them is built here from
/// those specifications ([`tack_on_reader`]). When the declaration asks for an
/// opaque wrapper ([`ArgDef::opaque_wrapper`]), the chosen parser is wrapped
/// so that what it reads is placed under one ([`OpaqueWrapperReader`]). The
/// declared mode and scoped packages apply around the parser, whatever it is,
/// as the state change of the argument's specification, so a parser of a
/// feature's own receives them exactly as a standard kind does.
fn argument_spec_of(
    arg: &ArgDef,
    arg_index: usize,
    field: Option<&str>,
    home: &mut Package<Flm>,
    def: &ElementDef,
    scopes: &Scopes<'_>,
    errors: &mut Vec<MintError>,
) -> Arc<ArgumentSpec<Flm>> {
    let parser: Arc<dyn ArgumentParser<Flm>> = match arg.kind.tack_on_fields() {
        Some(fields) => Arc::new(tack_on_reader(fields, home, def, arg_index, scopes, errors)),
        None => arg.kind.clone(),
    };
    let parser: Arc<dyn ArgumentParser<Flm>> = if arg.opaque_wrapper {
        Arc::new(OpaqueWrapperReader::new(parser))
    } else {
        parser
    };
    let spec = match arg.name.as_deref() {
        Some(name) => ArgumentSpec::new(parser, name),
        None => ArgumentSpec::new_unnamed(parser),
    };
    let unknown = |package: &Name| MintError::UnknownScopedPackage {
        element: def.element.name.clone(),
        owner: def.owner.clone(),
        argument: Some(arg_index),
        field: field.map(str::to_string),
        package: package.clone(),
    };
    let spec = match region_delta(arg.mode, &arg.scoped, &def.owner, scopes, unknown, errors) {
        Some(delta) => spec.with_state_delta(delta),
        None => spec,
    };
    Arc::new(spec)
}

/// Builds the reader of a tack-on argument, the `arg_index`-th of `def`:
/// techy's tack-on reader, configured with the specification of each of
/// `fields`, which [`field_spec`] builds into `home`.
// Why the minting, not the argument's parser object, builds this reader: see
// decision rationale at [§flm-dd-dr:lang-tack-on-reader].
fn tack_on_reader(
    fields: &[TackOnField],
    home: &mut Package<Flm>,
    def: &ElementDef,
    arg_index: usize,
    scopes: &Scopes<'_>,
    errors: &mut Vec<MintError>,
) -> TackOnFieldsArgumentParser<Flm> {
    let mut parser = TackOnFieldsArgumentParser::<Flm>::new(FlmCallableType::Macro);
    for field in fields {
        let spec = field_spec(field, home, def, arg_index, scopes, errors);
        parser = if field.repeatable {
            parser.with_repeatable_field(field.name.as_str(), spec)
        } else {
            parser.with_field(field.name.as_str(), spec)
        };
    }
    parser
}

// --- package minting ----------------------------------------------------------------

/// The result of [`mint_packages`]: the techy packages and the fallback provider
/// that the language build installs, the table of registered command names,
/// and the problems found.
pub struct MintedProviders {
    /// The features' techy packages, in registration order, outermost first,
    /// so a later feature's package shadows an earlier one's. A feature has one
    /// package per distinct set of parsing modes its declarations list (see
    /// the [module documentation](self)), in the order those sets first
    /// appear.
    pub packages: Vec<Arc<Package<Flm>>>,
    /// The scoped packages, keyed by the name by which an environment body or
    /// an argument refers to them.
    pub scoped: BTreeMap<Name, Arc<Package<Flm>>>,
    /// The fallback provider, which resolves a command that no declaration
    /// defines. It always holds at least the language's own two fallbacks.
    pub fallback: Arc<FlmFallbackProvider>,
    /// The techy package holding the specifications of tack-on fields
    /// ([`FIELDS_PACKAGE_NAME`]). It is never on the scope stack; the language
    /// build lists it so that a [reading session](crate::dump::session) can
    /// find those specifications when it reads a serialized tree.
    pub fields: Arc<Package<Flm>>,
    /// Every command name the features' techy packages register, with the
    /// feature whose definition applies: the one registered last.
    pub symbols: BTreeMap<Symbol, FeatureName>,
    /// The problems found; empty when every declaration was sound.
    pub errors: Vec<MintError>,
}

impl fmt::Debug for MintedProviders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MintedProviders")
            .field("packages", &self.packages.len())
            .field("scoped", &self.scoped.keys().collect::<Vec<_>>())
            .field("fallback", &self.fallback)
            .field("symbols", &self.symbols.len())
            .field("errors", &self.errors)
            .finish()
    }
}

/// One feature's definition of one name, and the parsing modes it is
/// available in.
struct Definer {
    owner: FeatureName,
    modes: Vec<Mode>,
}

/// The features that own the element declarations `elements`, in the order
/// [`mint_packages`] puts their techy packages on the scope stack: the order
/// of each feature's first declaration in `elements`. A later feature's
/// package shadows an earlier one's.
///
/// The registry's retained declarations
/// ([`Declarations`](crate::core::registry::Declarations)) are kept in this
/// order too, so that the last declaration of a name is the one in force.
pub(crate) fn feature_stack_order(elements: &[ElementDef]) -> Vec<FeatureName> {
    let mut owners: Vec<FeatureName> = Vec::new();
    for def in elements {
        if !owners.contains(&def.owner) {
            owners.push(def.owner.clone());
        }
    }
    owners
}

/// Builds every techy package and the fallback provider from the features'
/// declarations, and reports the declarations that do not fit.
///
/// `elements`, `fallbacks`, `scoped_packages` and `overrides` are the
/// declarations of every feature, in registration order. `explicitly_before`
/// holds the pairs `(a, b)` of features such that the feature order places `a`
/// explicitly before `b` ([`crate::core::registry::order`]); it decides which
/// features may define one name without an override. `peer_scoped_packages`
/// lists, as (feature, name), the scoped packages each feature declared as a
/// peer: that feature's bodies and arguments may refer to such a package even
/// when no feature declares it, in which case the reference adds nothing to
/// the parsing state and nothing is reported here. `structural` supplies the
/// elements that the language's own two fallbacks carry
/// (`flm.unknown.macro`, `flm.unknown.environment`).
///
/// It never fails: every problem is collected in
/// [`MintedProviders::errors`] as a [`MintError`], and the rest is built. The
/// rules (the order of the packages, how two features that define one name are
/// handled, the two packages kept off the scope stack) are in the [module
/// documentation](self).
///
/// The language build ([`build_language`](crate::lang::build_language)) calls
/// it with the registry's declarations. Call it directly only to build techy
/// packages from declarations outside a registry, as the `userdefs` feature
/// does for the language of its templates
/// ([`TemplateLanguage`](crate::lang::rewrites::userdefs::TemplateLanguage)).
pub fn mint_packages(
    elements: &[ElementDef],
    fallbacks: &[FallbackDef],
    scoped_packages: &[ScopedPackageDef],
    overrides: &[OverrideDef],
    explicitly_before: &BTreeSet<(FeatureName, FeatureName)>,
    peer_scoped_packages: &[(FeatureName, Name)],
    structural: &StructuralElements,
) -> MintedProviders {
    let mut errors = Vec::new();

    // Scoped packages first — each after the scoped packages its bodies name —
    // so the environment bodies referring to them by name find them minted; then
    // the features' own packages, in registration order, one per set of parsing
    // modes a feature's declarations name. Both are minted inside the fields
    // package's builder, which receives the specs of every tack-on field.
    // Why a feature may have several packages: see decision rationale at
    // [§flm-dd-dr:lang-feature-packages].
    let declared: BTreeSet<Name> = scoped_packages.iter().map(|p| p.name.clone()).collect();
    let peers: BTreeSet<(FeatureName, Name)> = peer_scoped_packages.iter().cloned().collect();
    let order = scoped_mint_order(scoped_packages, &mut errors);
    let owners = feature_stack_order(elements);
    for owner in &owners {
        report_redeclarations(owner, elements, &mut errors);
    }
    let mut scoped: BTreeMap<Name, Arc<Package<Flm>>> = BTreeMap::new();
    let mut packages = Vec::with_capacity(owners.len());
    // The owner of each minted package, in the same order: a feature has one
    // package per distinct set of parsing modes its declarations name, so the
    // two lists are no longer one-to-one.
    let mut package_owners: Vec<FeatureName> = Vec::with_capacity(owners.len());
    let fields = Package::new_shared(FIELDS_PACKAGE_NAME, |fields| {
        for index in order {
            let package = &scoped_packages[index];
            let minted = Package::new_shared(package.name.as_str(), |pkg| {
                for def in &package.elements {
                    let scopes = Scopes { minted: &scoped, declared: &declared, peers: &peers };
                    insert_element(pkg, def, &scopes, &mut errors, fields);
                }
            });
            scoped.insert(package.name.clone(), minted);
        }
        for owner in &owners {
            let groups = mode_groups(owner, elements);
            for (position, group) in groups.iter().enumerate() {
                let name = package_name(owner, position);
                let minted = Package::new_shared(name.as_str(), |pkg| {
                    for def in group {
                        let scopes = Scopes { minted: &scoped, declared: &declared, peers: &peers };
                    insert_element(pkg, def, &scopes, &mut errors, fields);
                    }
                });
                packages.push(minted);
                package_owners.push(owner.clone());
            }
        }
    });

    // Clash detection over the packages' own symbol enumeration, per mode.
    let mut symbols: BTreeMap<Symbol, FeatureName> = BTreeMap::new();
    let mut definers: BTreeMap<Symbol, Vec<Definer>> = BTreeMap::new();
    for (owner, package) in package_owners.iter().zip(&packages) {
        for &callable_type in FlmCallableType::ALL {
            let kind = match callable_type {
                FlmCallableType::Macro => CallableKind::Macro,
                FlmCallableType::Environment => CallableKind::Environment,
                FlmCallableType::Specials => CallableKind::Specials,
                _ => continue,
            };
            for &mode in Mode::ALL {
                let Some(entries) = package.iter_symbols(callable_type, mode) else {
                    continue;
                };
                for entry in entries {
                    let symbol = Symbol { kind, name: entry.name.to_string() };
                    let list = definers.entry(symbol).or_default();
                    match list.iter_mut().find(|definer| &definer.owner == owner) {
                        Some(definer) => {
                            if !definer.modes.contains(&mode) {
                                definer.modes.push(mode);
                            }
                        }
                        None => list.push(Definer { owner: owner.clone(), modes: vec![mode] }),
                    }
                }
            }
        }
    }
    for (symbol, definers) in &definers {
        let last = definers.last().expect("a symbol has at least one definer");
        symbols.insert(symbol.clone(), last.owner.clone());
        // Each parsing mode on its own: the features that define the name in
        // that mode, and the last of them, whose definition applies there.
        // Only that feature's override is consulted; the feature order allows
        // the collision when it places that feature explicitly after every
        // other. See decision rationale at [§flm-dd-dr:feat-order].
        for &mode in Mode::ALL {
            let owners_of: Vec<FeatureName> = definers
                .iter()
                .filter(|definer| definer.modes.contains(&mode))
                .map(|definer| definer.owner.clone())
                .collect();
            let [first, .., winner] = owners_of.as_slice() else {
                continue;
            };
            let overridden = overrides.iter().any(|o| {
                o.owner == *winner && o.kind == symbol.kind && o.name.as_str() == symbol.name
            });
            let ordered = owners_of[..owners_of.len() - 1]
                .iter()
                .all(|other| explicitly_before.contains(&(other.clone(), winner.clone())));
            if overridden || ordered {
                continue;
            }
            errors.push(MintError::Clash {
                kind: ClashKind::Symbol,
                symbol: symbol.clone(),
                modes: vec![mode],
                first: first.clone(),
                winner: winner.clone(),
                owners: owners_of,
            });
        }
    }

    // The fallbacks: the features' first (a later one overlapping an earlier one
    // of the same kind is a clash and is not installed), then the language
    // layer's own two recovery fallbacks, each for every mode. Being last, they
    // are consulted only where no feature answers. See the design rationale at
    // [§flm-dd-dr:err-unknown-callables].
    let mut pending: Vec<PendingFallback> = Vec::new();
    for def in fallbacks {
        let overlapping: Vec<Mode> = Mode::ALL
            .iter()
            .copied()
            .filter(|mode| {
                def.modes.contains(mode)
                    && pending.iter().any(|p| p.kind == def.kind && p.modes.contains(mode))
            })
            .collect();
        if !overlapping.is_empty() {
            let mut owners_of: Vec<FeatureName> = Vec::new();
            for earlier in &pending {
                let overlaps = earlier.kind == def.kind
                    && earlier.modes.iter().any(|mode| overlapping.contains(mode));
                if overlaps && !owners_of.contains(&earlier.owner) {
                    owners_of.push(earlier.owner.clone());
                }
            }
            owners_of.push(def.owner.clone());
            let first = owners_of[0].clone();
            errors.push(MintError::Clash {
                kind: ClashKind::Fallback,
                symbol: Symbol { kind: def.kind.callable_kind(), name: def.kind.to_string() },
                modes: overlapping,
                owners: owners_of,
                first: first.clone(),
                winner: first,
            });
            continue;
        }
        pending.push(PendingFallback {
            owner: def.owner.clone(),
            modes: def.modes.clone(),
            kind: def.kind,
            element: Arc::clone(&def.element),
            recovery: false,
        });
    }
    pending.push(PendingFallback {
        owner: FeatureName::from(LANGUAGE_OWNER),
        modes: Mode::ALL.to_vec(),
        kind: FallbackKind::Macro,
        element: Arc::clone(&structural.unknown_macro),
        recovery: true,
    });
    pending.push(PendingFallback {
        owner: FeatureName::from(LANGUAGE_OWNER),
        modes: Mode::ALL.to_vec(),
        kind: FallbackKind::Environment,
        element: Arc::clone(&structural.unknown_environment),
        recovery: true,
    });
    let fallback = Arc::new(FlmFallbackProvider::build(&pending));

    MintedProviders { packages, scoped, fallback, fields, symbols, errors }
}

// --- scoped package ordering ------------------------------------------------------------

/// Returns the scoped packages the declarations of `package` refer to: in an
/// environment's body, and in any argument, a tack-on field's own arguments
/// included.
fn scoped_dependencies(package: &ScopedPackageDef) -> Vec<&Name> {
    /// Collects the packages an argument list refers to, a tack-on field's
    /// arguments included.
    fn named_by<'d>(args: &'d [ArgDef], out: &mut Vec<&'d Name>) {
        for arg in args {
            out.extend(arg.scoped.iter());
            if let Some(fields) = arg.kind.tack_on_fields() {
                for field in fields {
                    named_by(&field.args, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for def in &package.elements {
        match &def.syntax {
            SyntaxDef::Macro { args } | SyntaxDef::Inclusion { args } | SyntaxDef::Specials { args, .. } => named_by(args, &mut out),
            SyntaxDef::Environment { args, body } => {
                named_by(args, &mut out);
                out.extend(body.scoped.iter());
            }
            SyntaxDef::Synthesized => {}
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Unvisited,
    Open,
    Done,
}

/// Returns the order in which the scoped packages are built: each after the
/// ones its declarations refer to. The traversal is depth-first and follows
/// declaration order, so the result is the same on every run.
///
/// A package name declared twice is reported, and only the first declaration
/// is built; a cycle is reported once and broken at the reference that
/// closes it.
fn scoped_mint_order(packages: &[ScopedPackageDef], errors: &mut Vec<MintError>) -> Vec<usize> {
    let mut by_name: BTreeMap<&Name, usize> = BTreeMap::new();
    for (index, package) in packages.iter().enumerate() {
        match by_name.get(&package.name) {
            Some(&first) => {
                let owners = vec![packages[first].owner.clone(), package.owner.clone()];
                errors.push(MintError::Clash {
                    kind: ClashKind::ScopedPackage,
                    symbol: Symbol { kind: CallableKind::Macro, name: package.name.to_string() },
                    modes: Vec::new(),
                    first: owners[0].clone(),
                    winner: owners[0].clone(),
                    owners,
                });
            }
            None => {
                by_name.insert(&package.name, index);
            }
        }
    }
    let mut marks = vec![Mark::Unvisited; packages.len()];
    let mut order = Vec::with_capacity(packages.len());
    let mut path = Vec::new();
    for index in 0..packages.len() {
        if by_name.get(&packages[index].name) == Some(&index) {
            visit_scoped(index, packages, &by_name, &mut marks, &mut path, &mut order, errors);
        }
    }
    order
}

/// One step of that ordering: visits the packages this one refers to, then
/// places it.
fn visit_scoped(
    index: usize,
    packages: &[ScopedPackageDef],
    by_name: &BTreeMap<&Name, usize>,
    marks: &mut [Mark],
    path: &mut Vec<usize>,
    order: &mut Vec<usize>,
    errors: &mut Vec<MintError>,
) {
    match marks[index] {
        Mark::Done => return,
        Mark::Open => {
            let start = path.iter().position(|&open| open == index).unwrap_or(0);
            let cycle = path[start..].iter().map(|&open| packages[open].name.clone()).collect();
            errors.push(MintError::ScopedPackageCycle { packages: cycle });
            return;
        }
        Mark::Unvisited => {}
    }
    marks[index] = Mark::Open;
    path.push(index);
    for name in scoped_dependencies(&packages[index]) {
        if let Some(&dependency) = by_name.get(name) {
            visit_scoped(dependency, packages, by_name, marks, path, order, errors);
        }
    }
    path.pop();
    marks[index] = Mark::Done;
    order.push(index);
}

// --- a feature's packages ---------------------------------------------------------------

/// Returns the parsing modes a declaration is available in
/// ([`ElementDef::modes`]), as positions in [`Mode::ALL`].
fn visible_modes(def: &ElementDef) -> Vec<usize> {
    (0..Mode::ALL.len()).filter(|index| def.modes.contains(&Mode::ALL[*index])).collect()
}

/// Returns the command names one declaration registers, as symbols: the
/// declaration's names for a macro or an environment, the trigger for a
/// special character sequence, nothing for an element with no syntax.
fn declared_symbols(def: &ElementDef) -> Vec<Symbol> {
    let Some(kind) = def.syntax.kind() else { return Vec::new() };
    match &def.syntax {
        SyntaxDef::Specials { trigger, .. } => {
            vec![Symbol { kind, name: trigger.clone() }]
        }
        _ => def
            .names
            .iter()
            .map(|name| Symbol { kind, name: name.to_string() })
            .collect(),
    }
}

/// Reports every name `owner` declares twice in parsing modes that overlap.
/// Both declarations of such a pair cannot apply where the modes overlap; a
/// pair whose modes are disjoint is sound and is built into two packages
/// ([`mode_groups`]).
fn report_redeclarations(
    owner: &FeatureName,
    elements: &[ElementDef],
    errors: &mut Vec<MintError>,
) {
    let mut seen: Vec<(Symbol, Vec<usize>)> = Vec::new();
    for def in elements.iter().filter(|def| &def.owner == owner) {
        let modes = visible_modes(def);
        for symbol in declared_symbols(def) {
            match seen.iter_mut().find(|(earlier, _)| earlier == &symbol) {
                Some((_, earlier_modes)) => {
                    let overlapping: Vec<Mode> = modes
                        .iter()
                        .filter(|index| earlier_modes.contains(index))
                        .map(|index| Mode::ALL[*index])
                        .collect();
                    if !overlapping.is_empty() {
                        errors.push(MintError::Redeclared {
                            symbol: symbol.clone(),
                            owner: owner.clone(),
                            modes: overlapping,
                        });
                    }
                    for index in &modes {
                        if !earlier_modes.contains(index) {
                            earlier_modes.push(*index);
                        }
                    }
                }
                None => seen.push((symbol, modes.clone())),
            }
        }
    }
}

/// Returns the declarations of `owner`, grouped by the set of parsing modes
/// they list, in the order the sets first appear. Each group becomes one
/// package, and the first group's package has the feature's own name
/// ([`package_name`]).
///
/// Two declarations that list the same modes in a different order, or one
/// mode twice, are grouped together.
fn mode_groups<'e>(owner: &FeatureName, elements: &'e [ElementDef]) -> Vec<Vec<&'e ElementDef>> {
    let mut keys: Vec<Vec<usize>> = Vec::new();
    let mut groups: Vec<Vec<&ElementDef>> = Vec::new();
    for def in elements.iter().filter(|def| &def.owner == owner) {
        let key = visible_modes(def);
        match keys.iter().position(|seen| seen == &key) {
            Some(position) => groups[position].push(def),
            None => {
                keys.push(key);
                groups.push(vec![def]);
            }
        }
    }
    groups
}

/// Returns the name of a feature's package at `position` among its packages:
/// the feature's own name for the first, which is the only one a feature with
/// one set of parsing modes has, and `<feature>#<n>` for the rest, with `n`
/// counting from 2.
///
/// The names must differ, because a serialized tree records the name of the
/// package a specification came from and is read back by that name.
fn package_name(owner: &FeatureName, position: usize) -> String {
    match position {
        0 => owner.to_string(),
        other => format!("{owner}#{}", other + 1),
    }
}

// --- element insertion ------------------------------------------------------------------

/// Builds one element declaration into `pkg`, one specification per name it is
/// written under. `fields` is the package that receives the specifications of
/// its tack-on fields.
fn insert_element(
    pkg: &mut Package<Flm>,
    def: &ElementDef,
    scopes: &Scopes<'_>,
    errors: &mut Vec<MintError>,
    fields: &mut Package<Flm>,
) {
    match &def.syntax {
        SyntaxDef::Macro { args } => {
            let args = argument_specs_of(args, None, fields, def, scopes, errors);
            let syntax: Arc<dyn CallableSpec<Flm>> = Arc::new(MacroSpec::<Flm>::new(args.specs));
            insert_callables(pkg, def, CallableKind::Macro, &syntax, &args.layouts, errors);
        }
        SyntaxDef::Inclusion { args } => {
            let args = argument_specs_of(args, None, fields, def, scopes, errors);
            match InputSpec::new(args.specs) {
                Ok(spec) => {
                    let syntax: Arc<dyn CallableSpec<Flm>> = Arc::new(spec);
                    insert_callables(pkg, def, CallableKind::Macro, &syntax, &args.layouts, errors);
                }
                Err(detail) => errors.push(MintError::NoReferenceArgument {
                    owner: def.owner.clone(),
                    element: def.element.name.clone(),
                    detail,
                }),
            }
        }
        SyntaxDef::Specials { trigger, args } => {
            let args = argument_specs_of(args, None, fields, def, scopes, errors);
            let syntax: Arc<dyn CallableSpec<Flm>> =
                Arc::new(SpecialsSpec::<Flm>::new(args.specs));
            insert_specials(pkg, def, trigger, &syntax, &args.layouts, errors);
        }
        SyntaxDef::Environment { args, body } => {
            let args = argument_specs_of(args, None, fields, def, scopes, errors);
            let environment =
                FlmEnvironment::new(args.specs, Arc::clone(&def.element), body.clone())
                    .with_arg_layouts(args.layouts);
            insert_environment(pkg, def, environment, body, scopes, errors);
        }
        SyntaxDef::Synthesized => {
            if !def.names.is_empty() {
                errors.push(MintError::SynthesizedWithNames {
                    owner: def.owner.clone(),
                    element: def.element.name.clone(),
                });
            }
        }
    }
}

/// Inserts a macro specification under each of the declaration's names,
/// restricted to its parsing modes, recording the layouts of its arguments.
fn insert_callables(
    pkg: &mut Package<Flm>,
    def: &ElementDef,
    kind: CallableKind,
    syntax: &Arc<dyn CallableSpec<Flm>>,
    layouts: &[ArgLayout],
    errors: &mut Vec<MintError>,
) {
    if def.names.is_empty() {
        errors.push(no_names(def));
        return;
    }
    let callable_type = kind.callable_type();
    for name in &def.names {
        let mut callable = FlmCallable::new(Arc::clone(syntax), Arc::clone(&def.element))
            .with_arg_layouts(layouts.to_vec());
        if let Some(provenance) = pkg.provenance_for(callable_type, name.as_str()) {
            callable = callable.with_provenance(provenance);
        }
        insert_in_modes(pkg, callable_type, name, callable, def);
    }
}

/// Inserts a special-character-sequence specification under the declaration's
/// trigger, restricted to its parsing modes, recording the layouts of its
/// arguments.
fn insert_specials(
    pkg: &mut Package<Flm>,
    def: &ElementDef,
    trigger: &str,
    syntax: &Arc<dyn CallableSpec<Flm>>,
    layouts: &[ArgLayout],
    errors: &mut Vec<MintError>,
) {
    if !def.names.is_empty() {
        errors.push(MintError::SpecialsWithNames {
            owner: def.owner.clone(),
            trigger: trigger.to_string(),
        });
    }
    let mut callable = FlmCallable::new(Arc::clone(syntax), Arc::clone(&def.element))
        .with_arg_layouts(layouts.to_vec());
    if let Some(provenance) = pkg.provenance_for_specials(FlmCallableType::Specials, trigger) {
        callable = callable.with_provenance(provenance);
    }
    pkg.insert_specials_in_modes(FlmCallableType::Specials, trigger, callable, Some(def.modes.clone()));
}

/// Inserts an environment under each of the declaration's names, with the change
/// of parsing state its body declaration asks for.
fn insert_environment(
    pkg: &mut Package<Flm>,
    def: &ElementDef,
    mut environment: FlmEnvironment,
    body: &BodyDef,
    scopes: &Scopes<'_>,
    errors: &mut Vec<MintError>,
) {
    if def.names.is_empty() {
        errors.push(no_names(def));
        return;
    }
    if body.opaque_wrapper && body.layout != BodyLayout::FromRole {
        errors.push(MintError::LayoutOnOpaqueWrappedBody {
            owner: def.owner.clone(),
            element: def.element.name.clone(),
        });
    }
    let unknown = |package: &Name| MintError::UnknownScopedPackage {
        element: def.element.name.clone(),
        owner: def.owner.clone(),
        argument: None,
        field: None,
        package: package.clone(),
    };
    if let Some(delta) = region_delta(body.mode, &body.scoped, &def.owner, scopes, unknown, errors) {
        environment = environment.with_body_delta(delta);
    }
    let spec = environment.into_spec();
    for name in &def.names {
        let stamped = match pkg.provenance_for(FlmCallableType::Environment, name.as_str()) {
            Some(provenance) => spec.clone().with_provenance(provenance),
            None => spec.clone(),
        };
        insert_in_modes(pkg, FlmCallableType::Environment, name, stamped, def);
    }
}

/// Returns the error for a macro or environment declaration that lists no
/// name.
fn no_names(def: &ElementDef) -> MintError {
    MintError::NoNames { owner: def.owner.clone(), element: def.element.name.clone() }
}

/// Inserts a specification under one name, restricted to the declaration's
/// parsing modes.
fn insert_in_modes<M>(
    pkg: &mut Package<Flm>,
    callable_type: FlmCallableType,
    name: &Name,
    spec: impl techy::core::specs::IntoCallableSpec<Flm, M>,
    def: &ElementDef,
) {
    pkg.insert_in_modes(callable_type, name.as_str(), spec, Some(def.modes.clone()));
}

/// Returns the change of parsing state a declared parsing mode asks for, or
/// `None` when the declaration keeps the mode of the place where the command
/// is written.
///
/// This is the only place where a declared mode becomes a change of parsing
/// state, for an argument ([`ArgDef::mode`]) and for an environment's body
/// ([`BodyDef::mode`]) alike, through [`region_delta`]. `Math` enters
/// mathematics; `Text` restores the innermost enclosing non-mathematics
/// context, with its mode and its token rules. That change makes `\text{…}`
/// inside mathematics hold ordinary document content, and changes nothing
/// outside mathematics.
fn mode_delta(mode: Option<Mode>) -> Option<ParsingStateDelta<Flm>> {
    match mode {
        Some(Mode::Math) => Some(ParsingStateDelta::new().mode(Mode::Math)),
        Some(Mode::Text) => Some(ParsingStateDelta::new().event(Event::ExitMathContext)),
        _ => None,
    }
}

/// Returns the change of parsing state that the declaration of an
/// environment's body ([`BodyDef`]) or of an argument ([`ArgDef`]) asks for:
/// the mode it enters and the scoped packages it adds to the scope stack, in
/// the order they are listed. Returns `None` when the declaration asks for no
/// change.
///
/// A scoped package that is declared but not yet built (one caught in a
/// cycle, which is reported separately) is skipped without a second report.
/// So is a package that no feature declares and that `owner`, the feature
/// whose declaration refers to it, declared as a peer; the registry build
/// reports that one. Any other package that no feature declares is reported
/// with the error `unknown` builds.
fn region_delta(
    mode: Option<Mode>,
    packages: &[Name],
    owner: &FeatureName,
    scopes: &Scopes<'_>,
    unknown: impl Fn(&Name) -> MintError,
    errors: &mut Vec<MintError>,
) -> Option<ParsingStateDelta<Flm>> {
    let mut delta = mode_delta(mode).unwrap_or_default();
    let mut any = mode.is_some();
    for name in packages {
        match scopes.minted.get(name) {
            Some(package) => {
                let provider: Arc<dyn SpecsProvider<Flm>> = package.clone();
                delta = delta.push_provider(provider);
                any = true;
            }
            None if scopes.declared.contains(name) => {}
            None if scopes.peers.contains(&(owner.clone(), name.clone())) => {}
            None => errors.push(unknown(name)),
        }
    }
    any.then_some(delta)
}
