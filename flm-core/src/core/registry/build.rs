//! The registry build: [`RegistryBuilder`] collects the features a registry
//! is built from, together with the embedding program's own service
//! contributions and workflow fields, and [`RegistryBuilder::build`] checks
//! all their declarations against each other and freezes the result into an
//! [`FlmRegistry`].
//!
//! # The five phases
//!
//! A build runs in five phases. Each phase collects every problem it finds
//! before the build stops:
//!
//! 1. **Collect.** The features take their final order: the build reduces
//!    the chains of the feature order to the features it holds and sorts the
//!    features by them, breaking ties by the order in which the features were
//!    added ([`super::order`]); chains that order a feature before itself are
//!    an error ([`BuildError::FeatureOrderCycle`]), and the features then keep
//!    the order in which they were added. Then every feature's
//!    [`register`](super::FlmFeature::register) method runs, in that order,
//!    and records its declarations in the collected declarations
//!    ([`Roster`]). Before a feature's `register` method runs, the build
//!    declares the option types that the feature's
//!    [`settings`](super::FlmFeature::settings) declaration lists; an error
//!    there is recorded like an error that `register` returns. Then the build
//!    assembles the service slots from the declared service slots, the
//!    contributions to them and the requirements that they be filled.
//! 2. **Finalize.** Every [`Finalizer`](super::Finalizer) that a feature
//!    registered runs. A finalizer can read the assembled services and
//!    declare further entities derived from what other features contributed.
//! 3. **Validate.** The build drops the members added to collections that no
//!    feature declares, and applies the fields registered for workflows by
//!    name. It resolves the [peer declarations](super::PeerName): a signature
//!    mismatch is an error; an unresolved declaration is a Note
//!    ([`UnresolvedPeer`]) among the build diagnostics; the lowering rules
//!    that depend on it are set aside (excluded from rule selection; see
//!    [`PeerName`]), and a stage that writes an unresolved peer table is not
//!    registered. Then the checks over all the collected declarations run,
//!    and the aspects that features attach to other features' elements are
//!    merged into the element values. Finally,
//!    every backend's native set is checked against the declared [native-set
//!    implications](super::NativeImplication), with the merged aspects.
//! 4. **Language.** The build creates the language that documents are parsed
//!    with (the commands and their syntax, and the parsing settings;
//!    [`FlmRegistry::lang`]) from the element, fallback, scoped-package and
//!    override declarations.
//! 5. **Late initialization.** Every [`LateInit`](super::LateInit) hook runs
//!    with the finished language. A hook that cannot do its work returns an
//!    error, and the phase fails with one [`BuildError::LateInit`] per such
//!    hook. A problem the hook reports about the configuration it prepares (for
//!    example, a template that does not parse) does not fail the build: it
//!    joins the registry's build diagnostics, which
//!    [`FlmRegistry::build_diagnostics`] returns.
//!
//! The build then computes the [registry digest](crate::model::digest) and
//! numbers the collected declarations into the frozen [`FlmRegistry`]. The
//! numbering can still find a reference that the checks let through; such an
//! error is reported under [`BuildPhase::Validate`].
//!
//! # Why a failing phase stops the build
//!
//! A phase that finds problems stops the build, and the phases after it do
//! not run. A later phase working on unsound declarations would report the
//! consequences of the earlier problems as problems of their own: for
//! example, a feature that failed to register leaves every reference to its
//! entities undeclared. The author would then read many consequences instead of the
//! few real problems. Within one phase, every error is collected and
//! reported together ([`RegistryBuildError`]), so one build shows every
//! problem of that phase. As a consequence, a problem that a later phase
//! would find is reported only once the problems of the earlier phases are
//! fixed.
//!
//! See also [`super::builder`] for the methods a feature declares through,
//! [`super::frozen`] for what the finished registry offers, and
//! [`crate::model::digest`] for the digest.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::lang::input::InputResolver;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::source::{Source, SourceSpan};

use crate::model::element::ElementSpec;
use crate::model::names::{CapName, Name, StageName};
use crate::model::names::FeatureName;
use crate::lang::{FlmSource, FlmSpan};
use crate::core::registry::builder::{BuildError, FeatureBuilder, HOST_FEATURE, Roster};
use crate::model::digest::Fnv1a64;
use crate::core::registry::feature::{FlmFeature, PackageEntry};
use crate::core::registry::order::{self, FeatureChain};
use crate::core::registry::entities::{PeerName, WorkflowDef};
use crate::core::registry::frozen::FlmRegistry;
use crate::core::registry::validate;
use crate::core::services::{Contribution, SlotTarget};
use crate::lang::guard::GuardPolicy;
use crate::lang::mint::{ArgDef, BodyDef, ElementDef, SyntaxDef};
use crate::lang::origin::FlmOrigin;
use crate::lang::rules::FlmParsing;
use crate::lang::structural::{self, StructuralElementSpecs, StructuralElements};
use crate::lang::{FlmDiagnostics, LangInputs, build_language};

/// Everything a registry build takes besides the features: how source text is
/// parsed, which structural elements the language starts from, the limits on
/// recursion and diagnostics, how `\input` finds files, and the identity of the
/// program.
///
/// The features decide what a registry can do; these inputs decide the
/// setting in which the features are built. Start from [`RegistryInputs::new`],
/// which sets every field to its default, change only the fields the program
/// needs, and pass the value to [`RegistryBuilder::build`].
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::RegistryInputs;
///
/// let mut inputs = RegistryInputs::new();
/// inputs.build_identity = Some("my-program 2.1".to_string());
/// inputs.diagnostics_limit = Some(200);
/// ```
pub struct RegistryInputs {
    /// The settings that decide how source text is split into tokens: whether
    /// comments are recognized and which character starts one, and whether
    /// `$` delimits mathematics.
    pub parsing: FlmParsing,
    /// The structural elements the language starts from, before features
    /// attach aspects to them. The structural elements are the elements of
    /// the node forms that no feature declares, such as text, groups and
    /// paragraphs ([`StructuralElements`]).
    pub structural: StructuralElementSpecs,
    /// The policy of the recursion guard, which stops a parse or a tree
    /// traversal that nests too deeply ([`crate::lang::guard`]).
    pub guard: GuardPolicy,
    /// The number of diagnostics one parse keeps; `None` keeps the default
    /// limit of techy, the parsing library FLM is built on.
    ///
    /// The registry's build diagnostics (what the late-initialization hooks
    /// report about the configuration they prepare) are a separate collection,
    /// which always keeps techy's default limit.
    pub diagnostics_limit: Option<usize>,
    /// The input resolver used by every parse under this registry: the host's
    /// implementation that finds and reads the file an `\input` command refers
    /// to ([`crate::lang::input`]). The library performs no input or output of
    /// its own, so a host that wants `\input` to work supplies a resolver, here
    /// or for one assembly ([`assemble`](crate::model::assemble::assemble)).
    /// With `None`, no file is included.
    pub input_resolver: Option<Arc<dyn InputResolver>>,
    /// A string that identifies the program this registry is built into, such
    /// as its version or a hash of its binary.
    ///
    /// It is an input of the [registry digest](crate::model::digest), so a
    /// cache that one program writes to disk is never read back by another.
    /// Two programs can register the same features and still behave
    /// differently, because a feature's behavior is code; the digest compares
    /// declarations but not code, and this string represents the difference.
    pub build_identity: Option<String>,
}

impl RegistryInputs {
    /// Inputs with every field at its default: techy's default parsing
    /// settings, the standard structural elements, the stack-budget guard
    /// policy ([`GuardPolicy::StackBudget`]), techy's default diagnostics
    /// limit, no input resolver and no build identity.
    pub fn new() -> RegistryInputs {
        RegistryInputs {
            parsing: FlmParsing::default(),
            structural: StructuralElementSpecs::default(),
            guard: GuardPolicy::StackBudget,
            diagnostics_limit: None,
            input_resolver: None,
            build_identity: None,
        }
    }
}

impl Default for RegistryInputs {
    fn default() -> RegistryInputs {
        RegistryInputs::new()
    }
}

impl fmt::Debug for RegistryInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryInputs")
            .field("parsing", &self.parsing)
            .field("guard", &self.guard)
            .field("diagnostics_limit", &self.diagnostics_limit)
            .field("build_identity", &self.build_identity)
            .finish_non_exhaustive()
    }
}

/// A phase of the registry build: the one that failed, in a
/// [`RegistryBuildError`]. The [module documentation](self) describes what
/// each phase does.
///
/// `Display` writes the phase as the lowercase word used in the message of a
/// [`RegistryBuildError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildPhase {
    /// The features take their final order, they register (the first
    /// registration round), and the service slots are assembled.
    Collect,
    /// The finalizers run (the second registration round).
    Finalize,
    /// The peer declarations are resolved, the checks over all the collected
    /// declarations run, and the aspect attachments are merged. An error found
    /// later, when the declarations are numbered after the late-initialization
    /// phase, is also reported under this phase.
    Validate,
    /// The language that documents are parsed with
    /// ([`FlmRegistry::lang`]) is built from the element declarations.
    Language,
    /// The late-initialization hooks run with the finished language.
    LateInit,
}

impl fmt::Display for BuildPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BuildPhase::Collect => "collect",
            BuildPhase::Finalize => "finalize",
            BuildPhase::Validate => "validate",
            BuildPhase::Language => "language",
            BuildPhase::LateInit => "late-init",
        })
    }
}

/// The error of a failed registry build: the phase that failed, and every
/// error that phase found.
///
/// A phase does not stop at its first problem, so `errors` usually holds
/// several entries and one build shows every problem of the failing phase.
/// The phases after it did not run, so the problems they would have found are
/// not listed; the [module documentation](self) explains why.
#[derive(Debug, Clone)]
pub struct RegistryBuildError {
    /// The phase that failed.
    pub phase: BuildPhase,
    /// Its errors, in the order they were found.
    pub errors: Vec<BuildError>,
}

impl RegistryBuildError {
    /// The errors as text, one per line, without the first line that
    /// `Display` adds. Two builds that find the same problems return the same
    /// text, so a test compares against this text.
    pub fn render(&self) -> String {
        self.errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
    }
}

impl fmt::Display for RegistryBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the registry could not be built: the {} phase found {} error(s):\n{}",
            self.phase,
            self.errors.len(),
            self.render()
        )
    }
}

impl std::error::Error for RegistryBuildError {}

/// The builder of a registry: it collects the features a registry is to be
/// built from, then builds the registry.
///
/// Add features one at a time with [`feature`](RegistryBuilder::feature), or a
/// whole package at once with [`use_package`](RegistryBuilder::use_package).
/// Add the chains of the feature order that the embedding program states with
/// [`feature_order`](RegistryBuilder::feature_order).
/// Add the embedding program's own contributions to service slots with
/// [`contribute`](RegistryBuilder::contribute) and
/// [`contribute_to`](RegistryBuilder::contribute_to), and the workflow fields it
/// sets with [`workflow_fields`](RegistryBuilder::workflow_fields). Nothing is
/// checked while the builder collects: [`build`](RegistryBuilder::build) runs
/// every check.
///
/// For a plain list of features and default inputs, call [`build_registry`]
/// instead.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{
///     BuildError, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder, RegistryInputs,
/// };
///
/// struct Empty;
/// impl FlmFeature for Empty {
///     fn info(&self) -> FeatureInfo { FeatureInfo::new("empty", "Empty") }
///     fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> { Ok(()) }
/// }
///
/// let mut builder = RegistryBuilder::new();
/// builder.feature(Arc::new(Empty));
/// let registry = builder.build(&RegistryInputs::new())?;
/// assert!(registry.has_feature("empty"));
/// # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
/// ```
#[derive(Default)]
pub struct RegistryBuilder {
    packages_seen: BTreeSet<Name>,
    features: Vec<Arc<dyn FlmFeature>>,
    feature_order: Vec<FeatureChain>,
    contributions: Vec<Contribution>,
    workflow_fields: Vec<WorkflowDef>,
}

impl RegistryBuilder {
    /// A builder that holds no feature, no chain of the feature order, no
    /// contribution and no workflow field.
    pub fn new() -> RegistryBuilder {
        RegistryBuilder::default()
    }

    /// Adds a package's features: first those of the packages it uses, then its
    /// own, each in the order the package lists them. Returns `&mut self`, for
    /// chaining.
    ///
    /// The chains of the feature order that each of these packages states
    /// ([`PackageEntry::feature_order`]) are added with its features, in the
    /// same order. The builder skips a package whose name it has already
    /// added, however often the package is passed here or used by other
    /// packages, so no package's features or chains are added twice.
    pub fn use_package(&mut self, entry: PackageEntry) -> &mut RegistryBuilder {
        if !self.packages_seen.insert(entry.name.clone()) {
            return self;
        }
        for used in entry.uses {
            self.use_package(used);
        }
        self.features.extend(entry.features);
        self.feature_order.extend(entry.feature_order);
        self
    }

    /// Adds one feature, after the features already added. Returns
    /// `&mut self`, for chaining.
    ///
    /// This method accepts two features of the same name; the build then
    /// fails with [`BuildError::DuplicateFeature`].
    pub fn feature(&mut self, feature: Arc<dyn FlmFeature>) -> &mut RegistryBuilder {
        self.features.push(feature);
        self
    }

    /// Adds one chain of the feature order on behalf of the embedding program,
    /// such as a chain a document states. Returns `&mut self`, for chaining.
    ///
    /// The build combines the chains given here with those of the packages
    /// ([`PackageEntry::feature_order`]) and places the features in their
    /// final order before they register ([`super::order`]). A chain may name
    /// features that the registry does not hold: the build ignores those
    /// names.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::registry::{FeatureChain, RegistryBuilder};
    ///
    /// let mut builder = RegistryBuilder::new();
    /// builder.feature_order(FeatureChain::parse(
    ///     "math-mathjax < userdefs",
    ///     "front matter of doc.flm, line 4",
    /// )?);
    /// # Ok::<(), flm_core::core::registry::ChainError>(())
    /// ```
    pub fn feature_order(&mut self, chain: FeatureChain) -> &mut RegistryBuilder {
        self.feature_order.push(chain);
        self
    }

    /// Contributes `imp` to the [service slot](crate::core::services) whose
    /// value type is `S`, on behalf of the embedding program instead of a
    /// feature. Returns `&mut self`, for chaining.
    ///
    /// With this method, a host supplies an implementation that a feature
    /// declared a service slot for and left to the embedding program, for example to
    /// read files or to fetch the data at an address. The contribution is
    /// attributed to [`HOST_FEATURE`] and comes after every contribution of a
    /// feature.
    ///
    /// Use [`contribute_to`](RegistryBuilder::contribute_to) to give the service
    /// slot by name instead of by value type.
    pub fn contribute<S: ?Sized + std::any::Any + Send + Sync>(
        &mut self,
        imp: Arc<S>,
    ) -> &mut RegistryBuilder {
        self.contributions.push(Contribution::new(
            HOST_FEATURE,
            SlotTarget::ByType(crate::model::options::TypeKey::of::<S>()),
            imp,
            false,
        ));
        self
    }

    /// Contributes `imp` to the [service slot](crate::core::services) called
    /// `slot`, on behalf of the embedding program. Returns `&mut self`, for
    /// chaining.
    ///
    /// This method does what [`contribute`](RegistryBuilder::contribute) does,
    /// for a service slot given by name instead of by value type.
    pub fn contribute_to<S: ?Sized + std::any::Any + Send + Sync>(
        &mut self,
        slot: impl Into<Name>,
        imp: Arc<S>,
    ) -> &mut RegistryBuilder {
        self.contributions.push(Contribution::new(
            HOST_FEATURE,
            SlotTarget::ByName(slot.into()),
            imp,
            false,
        ));
        self
    }

    /// Registers fields for the [workflow](WorkflowDef) called `def.name`, on
    /// behalf of the embedding program instead of a feature. Returns
    /// `&mut self`, for chaining.
    ///
    /// The build applies these fields as it applies those a feature registers
    /// with [`FeatureBuilder::workflow_fields`], and after all of them: every
    /// field that `def` declares replaces that field of the workflow of that
    /// name, and when no feature registers a workflow of that name, `def` is
    /// the workflow, attributed to [`HOST_FEATURE`]. A host that combines
    /// settings from several sources of its own (for example, files of
    /// settings that can change a workflow) uses this method to give the
    /// library the resulting fields, once per workflow, without knowing
    /// whether a feature registers a workflow of that name.
    pub fn workflow_fields(&mut self, def: WorkflowDef) -> &mut RegistryBuilder {
        self.workflow_fields.push(def);
        self
    }

    /// Builds the registry from the collected features and the `inputs`, and
    /// returns the frozen registry, ready to be shared by any number of
    /// documents, plans and runs.
    ///
    /// The [module documentation](self) describes the five phases of the
    /// build.
    ///
    /// # Errors
    ///
    /// A [`RegistryBuildError`] that gives the first phase that found
    /// problems and lists every problem that phase found. The phases after it
    /// did not run.
    pub fn build(self, inputs: &RegistryInputs) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
        // See the design rationale at [§flm-dd-dr:feat-build-phases].
        let mut roster = Roster::default();

        // --- phase: collect (round one) --------------------------------------------------
        // The features first take their final order, which every later step
        // follows. See decision rationale at [§flm-dd-dr:feat-order].
        // A cycle is reported with the phase's other errors; the features
        // then register in their implicit order.
        let placement = order::place(self.features, &self.feature_order);
        roster.errors.extend(placement.cycle);
        for feature in &placement.features {
            let info = feature.info();
            let name = info.name.clone();
            roster.features.push(info);
            let mut builder = FeatureBuilder::round_one(&mut roster, name.clone());
            let declared = match feature.settings() {
                Some(decl) => decl.declare(&mut builder),
                None => Ok(()),
            };
            if let Err(error) = declared.and_then(|()| feature.register(&mut builder)) {
                roster.errors.push(match error {
                    BuildError::Feature { feature, detail } => {
                        BuildError::Feature { feature, detail }
                    }
                    other => {
                        BuildError::Feature { feature: name.clone(), detail: other.to_string() }
                    }
                });
            }
        }
        roster.contributions.extend(self.contributions);
        let mut errors = std::mem::take(&mut roster.errors);
        let final_order: Vec<FeatureName> = roster.features.iter().map(|info| info.name.clone()).collect();
        let services = match crate::core::services::assemble(
            std::mem::take(&mut roster.slots),
            std::mem::take(&mut roster.contributions),
            std::mem::take(&mut roster.requirements),
            &final_order,
        ) {
            Ok(services) => Some(services),
            Err(service_errors) => {
                errors.extend(service_errors.into_iter().map(BuildError::Service));
                None
            }
        };
        if !errors.is_empty() {
            return Err(RegistryBuildError { phase: BuildPhase::Collect, errors });
        }
        let services = services.expect("assembled without errors");

        // --- phase: finalize (round two) -------------------------------------------------
        let finalizers = std::mem::take(&mut roster.finalizers);
        for (feature, finalizer) in finalizers {
            let mut builder = FeatureBuilder::round_two(&mut roster, feature.clone(), &services);
            if let Err(error) = finalizer.finalize(&mut builder) {
                roster.errors.push(match error {
                    BuildError::ContributionAfterAssembly { feature, action } => {
                        BuildError::ContributionAfterAssembly { feature, action }
                    }
                    BuildError::Feature { feature, detail } => {
                        BuildError::Feature { feature, detail }
                    }
                    other => {
                        BuildError::Feature { feature: feature.clone(), detail: other.to_string() }
                    }
                });
            }
        }
        let errors = std::mem::take(&mut roster.errors);
        if !errors.is_empty() {
            return Err(RegistryBuildError { phase: BuildPhase::Finalize, errors });
        }

        // --- phase: validate ---------------------------------------------------------------
        let mut build_diagnostics = FlmDiagnostics::new();
        drop_members_of_undeclared_collections(&mut roster);
        roster
            .workflow_fields
            .extend(self.workflow_fields.into_iter().map(|def| (FeatureName::from(HOST_FEATURE), def)));
        apply_workflow_fields(&mut roster);
        validate::merge_repeated_peers(&mut roster);
        // A value of an element type nobody declares is dropped before any
        // check sees it; one Note per feature and element type.
        for (feature, element) in validate::resolve_peer_element_values(&mut roster, &structural::NAMES) {
            let name = PeerName::ElementType(element);
            build_diagnostics.push(Diagnostic::note(
                UnresolvedPeer {
                    feature: feature.to_string(),
                    kind: name.kind().to_string(),
                    name: name.name().to_string(),
                    rules: Vec::new(),
                    stages: Vec::new(),
                },
                declarations_span(&feature),
            ));
        }
        // The peer declarations are resolved first: which of them stay
        // unresolved decides which rules are set aside, and the roster-wide
        // validation accepts a rule's reference to a name its own feature
        // declared as a peer.
        let validate::PeerResolution { errors: peer_errors, unresolved, set_aside, set_aside_stages } =
            validate::resolve_peers(&roster, &structural::NAMES);
        // A stage set aside is not registered: nothing sees it from here on.
        // See decision rationale at [§flm-dd-dr:feat-peer-table-write]
        remove_stages(&mut roster, &set_aside_stages);
        let mut errors = validate::validate(&roster, &structural::NAMES);
        errors.extend(peer_errors);
        for (feature, name, rules) in unresolved {
            let stages = set_aside_stages
                .iter()
                .filter(|(_, (owner, names))| *owner == feature && names.contains(&name))
                .map(|(stage, _)| stage.to_string())
                .collect();
            build_diagnostics.push(Diagnostic::note(
                UnresolvedPeer {
                    feature: feature.to_string(),
                    kind: name.kind().to_string(),
                    name: name.name().to_string(),
                    rules: rules.iter().map(ToString::to_string).collect(),
                    stages,
                },
                declarations_span(&feature),
            ));
        }
        // The peer contributions were resolved when the slots were assembled,
        // two phases earlier; the ones that found no slot are reported here,
        // with the peer declarations they belong beside. One Note per feature
        // and slot, however many values that feature offered to it: the other
        // peer declarations get one Note per name because a feature may name
        // one entity only once, and a slot holding many values must not be
        // noisier for holding them.
        let mut noted: BTreeSet<(&FeatureName, &Name)> = BTreeSet::new();
        for peer in services.peer_contributions().iter().filter(|peer| !peer.resolved) {
            if !noted.insert((&peer.owner, &peer.slot)) {
                continue;
            }
            let name = PeerName::Slot(peer.slot.clone());
            build_diagnostics.push(Diagnostic::note(
                UnresolvedPeer {
                    feature: peer.owner.to_string(),
                    kind: name.kind().to_string(),
                    name: name.name().to_string(),
                    rules: Vec::new(),
                    stages: Vec::new(),
                },
                declarations_span(&peer.owner),
            ));
        }
        let structural = match attach_aspects(&mut roster, inputs.structural.clone()) {
            Ok(structural) => Some(structural),
            Err(attach_errors) => {
                errors.extend(attach_errors);
                None
            }
        };
        if let Some(structural) = &structural {
            errors.extend(validate::native_implications(&roster, structural));
        }
        if !errors.is_empty() {
            return Err(RegistryBuildError { phase: BuildPhase::Validate, errors });
        }
        let structural = Arc::new(structural.expect("attached without errors"));

        // --- phase: the language --------------------------------------------------------------
        let lang = build_language(LangInputs {
            elements: roster.elements.clone(),
            fallbacks: roster.fallbacks.clone(),
            scoped_packages: roster.scoped_packages.clone(),
            peer_scoped_packages: roster
                .peer_scoped_packages
                .iter()
                .map(|record| (record.owner.clone(), record.name.clone()))
                .collect(),
            overrides: roster.overrides.clone(),
            explicitly_before: placement.explicitly_before,
            parsing: inputs.parsing.clone(),
            structural: Arc::clone(&structural),
            guard: inputs.guard,
            diagnostics_limit: inputs.diagnostics_limit,
            input_resolver: inputs.input_resolver.clone(),
        });
        if !lang.is_sound() {
            return Err(RegistryBuildError {
                phase: BuildPhase::Language,
                errors: lang.errors().iter().cloned().map(BuildError::Lang).collect(),
            });
        }

        // --- phase: late init ----------------------------------------------------------------
        let late_inits = std::mem::take(&mut roster.late_inits);
        let mut errors = Vec::new();
        for (feature, hook) in late_inits {
            if let Err(error) = hook.late_init(&lang, &mut build_diagnostics) {
                errors.push(BuildError::LateInit { feature, detail: error.detail });
            }
        }
        if !errors.is_empty() {
            return Err(RegistryBuildError { phase: BuildPhase::LateInit, errors });
        }

        // --- digest and freeze ----------------------------------------------------------------
        let digest = digest_of_roster(&roster, inputs, &structural);
        match FlmRegistry::assemble(
            roster,
            structural,
            lang,
            services,
            digest,
            build_diagnostics,
            set_aside,
        ) {
            Ok(registry) => Ok(Arc::new(registry)),
            Err(errors) => Err(RegistryBuildError { phase: BuildPhase::Validate, errors }),
        }
    }
}

/// Condition `flm.registry.unresolved-peer`: a feature made a [peer
/// declaration](super::PeerName) of an entity (a backend, a presentation
/// element, a table, a service slot, a scoped package, an element type or an
/// argument role) that no feature of the registry declares.
///
/// It is a Note among the registry's [build
/// diagnostics](FlmRegistry::build_diagnostics), never a failure: a feature
/// that ships a lowering rule for a backend this registry does not hold is
/// doing something ordinary. A name that stays unresolved on a registry that
/// does hold the other feature is misspelled in one of the two features.
///
/// Recovery: what the build does depends on the kind of entity.
///
/// - Every lowering rule of the feature that depends on the name is set aside:
///   listed on the registry, never selected by a plan and never counted by the
///   check that a backend can write every element; the rest of the feature
///   takes part as usual. [`FlmRegistry::set_aside_rules`] lists the rules set
///   aside. A rule that reads an unresolved peer table only optionally is not
///   among them: it runs and finds the table absent.
/// - Every stage of the feature that writes an unresolved peer table is set
///   aside too, and not registered at all; the Note lists it.
/// - For an unresolved service slot, every value the feature contributed to it
///   is dropped, and nothing else changes; one Note covers the service slot, however
///   many values the feature offered it, and [`FlmRegistry::peer_slots`] lists
///   every contribution.
/// - For an unresolved scoped package, every environment body and argument of
///   the feature that refers to it gets no definitions from it, so a command
///   the package would define is an unknown command there.
/// - For an unresolved element type, every value the feature declared of it is
///   dropped, so its commands are unknown commands; one Note covers the element
///   type, however many values the feature declared.
/// - For an unresolved argument role, nothing changes: the feature's arguments
///   keep the role, of which the registry holds no description.
///
/// A registry that holds the missing feature reports nothing and treats the
/// rules like any other.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.registry.unresolved-peer")]
pub struct UnresolvedPeer {
    /// The feature that made the peer declaration.
    pub feature: String,
    /// The kind of the entity the declaration refers to: `backend`, `element`,
    /// `table`, `slot`, `scoped package`, `element type` or `argument role`
    /// ([`PeerName::kind`]).
    pub kind: String,
    /// The name that resolved to nothing.
    pub name: String,
    /// The names of the rules of that feature that are set aside because they
    /// depend on the name, in registration order.
    pub rules: Vec<String>,
    /// The names of the stages of that feature that are set aside, and not
    /// registered, because they write the table of that name; in name order.
    pub stages: Vec<String>,
}

impl fmt::Display for UnresolvedPeer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "feature `{}` refers to the {} `{}` and expects another feature to declare it, \
             but no feature of this registry declares it",
            self.feature, self.kind, self.name
        )?;
        // An argument role changes nothing anywhere when it stays unresolved:
        // the condition is the whole message. The word is the one
        // `PeerName::kind` gives for `PeerName::ArgumentRole`.
        if self.kind == "argument role" {
            return Ok(());
        }
        f.write_str("; ")?;
        // A slot names no rule at all: what happened to the value is what a
        // reader needs to be told. The word is the one `PeerName::kind` gives
        // for `PeerName::Slot`.
        if self.kind == "slot" {
            return write!(f, "the value contributed to it is dropped");
        }
        // A scoped package names no rule either: what the feature's bodies and
        // arguments naming it see is what a reader needs. The word is the one
        // `PeerName::kind` gives for `PeerName::ScopedPackage`.
        if self.kind == "scoped package" {
            return write!(f, "the environment bodies and arguments that refer to it get no definitions from it");
        }
        // An element type names no rule either: the sentence states the
        // condition the feature's commands need. The word is the one
        // `PeerName::kind` gives for `PeerName::ElementType`.
        if self.kind == "element type" {
            return write!(f, "the feature's commands of that element type are not defined");
        }
        // An empty list is the ordinary case for a table only optional reads
        // name: nothing is set aside, and the sentence says what happens
        // instead rather than ending in `[]`.
        if self.rules.is_empty() && self.stages.is_empty() {
            return write!(f, "no rule of that feature depends on it");
        }
        if !self.rules.is_empty() {
            write!(f, "the rules depending on it are set aside: [{}]", self.rules.join(", "))?;
        }
        if !self.stages.is_empty() {
            if !self.rules.is_empty() {
                f.write_str("; ")?;
            }
            write!(f, "the stages writing it are set aside: [{}]", self.stages.join(", "))?;
        }
        Ok(())
    }
}

/// Removes from the collected declarations `roster` every stage whose name is
/// a key of `names`, with every ordering edge placed on it, and updates the
/// positions of the backends' stages so that they still refer to the same
/// stages. A backend's own stage is never removed, because it writes its
/// backend's tables.
fn remove_stages<T>(roster: &mut Roster, names: &BTreeMap<StageName, T>) {
    let backend_stages: BTreeSet<usize> = roster
        .backends
        .iter()
        .flat_map(|backend| [backend.lowering, backend.emit, backend.final_stage])
        .collect();
    let mut kept_before: Vec<usize> = Vec::with_capacity(roster.stages.len());
    let mut kept = 0;
    let mut index = 0;
    roster.stages.retain(|record| {
        kept_before.push(kept);
        let keep = backend_stages.contains(&index) || !names.contains_key(&record.decl.name);
        index += 1;
        if keep {
            kept += 1;
        }
        keep
    });
    for backend in &mut roster.backends {
        backend.lowering = kept_before[backend.lowering];
        backend.emit = kept_before[backend.emit];
        backend.final_stage = kept_before[backend.final_stage];
    }
    let registered: BTreeSet<StageName> = roster.stages.iter().map(|record| record.decl.name.clone()).collect();
    roster
        .orderings
        .retain(|(_, ordering)| registered.contains(&ordering.stage) || !names.contains_key(&ordering.stage));
}

/// The span that a build diagnostic about the declarations of `feature`
/// carries: the start of an empty synthetic source whose origin label gives
/// the feature, because a declaration made in code has no source text.
fn declarations_span(feature: &FeatureName) -> FlmSpan {
    let origin = FlmOrigin::synthetic(format!("declarations of feature `{feature}`"));
    let source: Arc<FlmSource> = Arc::new(Source::new(String::new()).with_origin(origin));
    SourceSpan::new(&source, 0..0)
}

/// Applies the fields registered for workflows by name
/// ([`FeatureBuilder::workflow_fields`](super::FeatureBuilder::workflow_fields),
/// then [`RegistryBuilder::workflow_fields`]) to the workflows of those names,
/// in registration order. Fields for a name under which no feature registers
/// a workflow become that workflow.
fn apply_workflow_fields(roster: &mut Roster) {
    for (owner, fields) in std::mem::take(&mut roster.workflow_fields) {
        match roster.workflows.iter_mut().find(|(_, def)| def.name == fields.name) {
            Some((_, def)) => def.replace_declared_fields(fields),
            None => roster.workflows.push((owner, fields)),
        }
    }
}

// See the design rationale at [§flm-dd-dr:feat-undeclared-collection].
/// Drops every member added to a collection that no feature of this registry
/// declares, so that the checks that follow see only members of declared
/// collections.
///
/// No diagnostic is recorded. A feature that ships a check adds it to a
/// collection another feature declares; dropping the member lets the feature
/// be registered without the other one, and reporting it would refuse
/// ordinary registries.
fn drop_members_of_undeclared_collections(roster: &mut Roster) {
    let declared: BTreeSet<CapName> =
        roster.collections.iter().map(|(_, def)| def.name.clone()).collect();
    roster.members.retain(|(_, collection, _)| declared.contains(collection));
}

impl fmt::Debug for RegistryBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryBuilder")
            .field("packages", &self.packages_seen)
            .field("features", &self.features.iter().map(|x| x.info().name).collect::<Vec<_>>())
            .field("contributions", &self.contributions.len())
            .field("workflow_fields", &self.workflow_fields.iter().map(|def| &def.name).collect::<Vec<_>>())
            .finish()
    }
}

/// Merges the aspects that features attach to elements into the element
/// values themselves, and returns the frozen structural elements.
///
/// An element value is shared and immutable, so an aspect cannot be added in
/// place: every value whose element type has attachments is copied with the
/// aspects added, and the new shared value replaces the old one everywhere
/// in the collected declarations `roster`. Values without attachments keep
/// their identity, so the usual case copies nothing. The aspects of the
/// structural elements are added to their specifications
/// (`structural_specs`), before these are frozen.
///
/// # Errors
///
/// One [`BuildError::AspectCollision`] for each aspect attached to an element
/// that already carries it, all returned together.
fn attach_aspects(
    roster: &mut Roster,
    mut structural_specs: StructuralElementSpecs,
) -> Result<StructuralElements, Vec<BuildError>> {
    let mut errors: Vec<BuildError> = Vec::new();
    let attachments = std::mem::take(&mut roster.attachments);
    if attachments.is_empty() {
        return Ok(StructuralElements::new(structural_specs));
    }

    // Structural elements: attach on the mutable specs.
    for attachment in &attachments {
        if let Some(spec) = structural_specs.get_mut(attachment.element.as_str()) {
            if spec
                .aspects
                .insert_erased(
                    attachment.aspect_id,
                    attachment.aspect_name,
                    Arc::clone(&attachment.imp),
                )
                .is_err()
            {
                errors.push(BuildError::AspectCollision {
                    feature: attachment.owner.clone(),
                    element: attachment.element.clone(),
                    aspect: attachment.aspect_name,
                });
            }
        }
    }

    // Feature elements: rebuild the values whose names have attachments.
    let mut rebuilt: std::collections::BTreeMap<usize, Arc<ElementSpec>> =
        std::collections::BTreeMap::new();
    let mut rebuild = |element: &Arc<ElementSpec>, errors: &mut Vec<BuildError>| {
        let key = Arc::as_ptr(element) as usize;
        if rebuilt.contains_key(&key) {
            return;
        }
        let mine: Vec<&_> = attachments.iter().filter(|a| a.element == element.name).collect();
        if mine.is_empty() {
            return;
        }
        let mut spec: ElementSpec = (**element).clone();
        for attachment in mine {
            if spec
                .aspects
                .insert_erased(
                    attachment.aspect_id,
                    attachment.aspect_name,
                    Arc::clone(&attachment.imp),
                )
                .is_err()
            {
                errors.push(BuildError::AspectCollision {
                    feature: attachment.owner.clone(),
                    element: attachment.element.clone(),
                    aspect: attachment.aspect_name,
                });
            }
        }
        rebuilt.insert(key, Arc::new(spec));
    };
    let mut visit_def = |def: &ElementDef, errors: &mut Vec<BuildError>| {
        rebuild(&def.element, errors);
        let args = match &def.syntax {
            SyntaxDef::Macro { args } | SyntaxDef::Inclusion { args } | SyntaxDef::Specials { args, .. } => args.as_slice(),
            SyntaxDef::Environment { args, .. } => args.as_slice(),
            SyntaxDef::Synthesized => &[],
        };
        for arg in args {
            for field in arg.kind.tack_on_fields().unwrap_or_default() {
                rebuild(field.element(), errors);
            }
        }
    };
    for def in &roster.elements {
        visit_def(def, &mut errors);
    }
    for package in &roster.scoped_packages {
        for def in &package.elements {
            visit_def(def, &mut errors);
        }
    }
    for fallback in &roster.fallbacks {
        rebuild(&fallback.element, &mut errors);
    }
    for record in &roster.presentation {
        rebuild(&record.element, &mut errors);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let rebuilt_of =
        |element: &Arc<ElementSpec>| rebuilt.get(&(Arc::as_ptr(element) as usize)).map(Arc::clone);
    let remap = |element: &mut Arc<ElementSpec>| {
        if let Some(new) = rebuilt_of(element) {
            *element = new;
        }
    };
    let remap_def = |def: &mut ElementDef| {
        remap(&mut def.element);
        let args = match &mut def.syntax {
            SyntaxDef::Macro { args } | SyntaxDef::Inclusion { args } | SyntaxDef::Specials { args, .. } => args.as_mut_slice(),
            SyntaxDef::Environment { args, .. } => args.as_mut_slice(),
            SyntaxDef::Synthesized => &mut [],
        };
        for arg in args {
            arg.remap_field_elements(rebuilt_of);
        }
    };
    for def in &mut roster.elements {
        remap_def(def);
    }
    for package in &mut roster.scoped_packages {
        for def in &mut package.elements {
            remap_def(def);
        }
    }
    for fallback in &mut roster.fallbacks {
        remap(&mut fallback.element);
    }
    for record in &mut roster.presentation {
        remap(&mut record.element);
    }
    Ok(StructuralElements::new(structural_specs))
}

// See the design rationale at [§flm-dd-dr:feat-digest] and, for what a
// feature's own configuration contributes, [§flm-dd-dr:feat-config-hash].
/// Computes the [registry digest](crate::model::digest): a hash of
/// everything about this build that can change what processing produces.
///
/// The inputs, in order: the versions of this crate and of techy, the host's
/// build identity, the parsing settings, the structural elements with their
/// aspects, every feature's name and version, the element declarations
/// (owner, callable names, syntax, element type name, aspects and modes; the
/// syntax covers the parser identity, mode, layout, opaque wrapper and scoped
/// packages of each argument and body, as [`digest_syntax`] lists), the
/// fallbacks, the scoped packages, the overrides, the rewrite stages with their
/// refinements, the ordering edges, the option declarations, and every
/// feature's own digest input. What stages compute and what backends emit are
/// not inputs: they follow from these inputs.
///
/// The only `Debug` texts used as inputs are those of plain derived data
/// (techy's parse mode, and the parsing, argument-layout and body
/// declarations), never of a value whose `Debug` text could contain a pointer
/// address, which would differ between two runs of the same program.
fn digest_of_roster(
    roster: &Roster,
    inputs: &RegistryInputs,
    structural: &StructuralElements,
) -> u64 {
    let mut hasher = Fnv1a64::new();
    hasher.write_str(env!("CARGO_PKG_VERSION"));
    hasher.write_str(techy::VERSION);
    hasher.write_str(inputs.build_identity.as_deref().unwrap_or(""));
    hasher.write_str(&format!("{:?}", inputs.parsing));
    for element in structural.all() {
        hasher.write_str(element.name.as_str());
        let mut aspects: Vec<&str> = element.aspects.entries().map(|(_, name, _)| name).collect();
        aspects.sort_unstable();
        for aspect in aspects {
            hasher.write_str(aspect);
        }
    }
    for info in &roster.features {
        hasher.write_str(info.name.as_str());
        hasher.write_str(info.version.as_deref().unwrap_or(""));
    }
    let digest_def = |hasher: &mut Fnv1a64, def: &ElementDef| {
        hasher.write_str(def.owner.as_str());
        for name in &def.names {
            hasher.write_str(name.as_str());
        }
        digest_syntax(hasher, &def.syntax);
        hasher.write_str(def.element.name.as_str());
        let mut aspects: Vec<&str> =
            def.element.aspects.entries().map(|(_, name, _)| name).collect();
        aspects.sort_unstable();
        for aspect in aspects {
            hasher.write_str(aspect);
        }
        hasher.write_str(&format!("{:?}", def.modes));
    };
    for def in &roster.elements {
        digest_def(&mut hasher, def);
    }
    for fallback in &roster.fallbacks {
        hasher.write_str(fallback.owner.as_str());
        for mode in &fallback.modes {
            hasher.write_str(&format!("{mode:?}"));
        }
        hasher.write_str(&fallback.kind.to_string());
        hasher.write_str(fallback.element.name.as_str());
    }
    for package in &roster.scoped_packages {
        hasher.write_str(package.owner.as_str());
        hasher.write_str(package.name.as_str());
        for def in &package.elements {
            digest_def(&mut hasher, def);
        }
    }
    for over in &roster.overrides {
        hasher.write_str(over.owner.as_str());
        hasher.write_str(&over.kind.to_string());
        hasher.write_str(over.name.as_str());
    }
    for record in &roster.stages {
        if let Some(rewrite) = &record.rewrite {
            hasher.write_str(rewrite.name.as_str());
            hasher.write_str(rewrite.publishes.as_str());
            hasher.write_str(&format!("{:?}", rewrite.refines));
            hasher.write_u64(u64::from(rewrite.default));
        }
    }
    for (owner, ordering) in &roster.orderings {
        hasher.write_str(owner.as_str());
        hasher.write_str(ordering.stage.as_str());
        hasher.write_str("after");
        for capability in &ordering.after {
            hasher.write_str(capability.as_str());
        }
        hasher.write_str("before");
        for capability in &ordering.before {
            hasher.write_str(capability.as_str());
        }
    }
    for decl in &roster.options {
        decl.key.write_digest(&mut hasher);
        hasher.write_str(decl.bind.label());
    }
    for input in &roster.digest_inputs {
        hasher.write_str(input.owner.as_str());
        hasher.write_bytes(&input.bytes);
        hasher.write_bytes(&[0xfe]);
    }
    hasher.value()
}

/// Adds the form of a declared syntax to the digest: its kind, its trigger
/// characters where it has some, and for each argument its name, its parser's
/// identity ([`FlmArgumentParser::identity`](crate::lang::mint::FlmArgumentParser::identity),
/// which for a tack-on argument covers each field's name, arguments and
/// repeatability), its mode, its layout, whether it is declared with an
/// opaque wrapper and its scoped packages; and for a body its layout, whether
/// it is declared with an opaque wrapper, its parser's identity
/// ([`FlmBodyParser::identity`](crate::lang::mint::FlmBodyParser::identity)),
/// its mode and its scoped packages. Documentation strings change no parse
/// and are not inputs.
///
/// A parser that a feature defines is an input through its identity; what
/// its code does beyond that is an input only through the feature's version
/// and the host's build identity.
fn digest_syntax(hasher: &mut Fnv1a64, syntax: &SyntaxDef) {
    match syntax {
        SyntaxDef::Macro { args } => {
            hasher.write_str("macro");
            digest_args(hasher, args);
        }
        SyntaxDef::Inclusion { args } => {
            hasher.write_str("inclusion");
            digest_args(hasher, args);
        }
        SyntaxDef::Environment { args, body } => {
            hasher.write_str("environment");
            digest_args(hasher, args);
            digest_body(hasher, body);
        }
        SyntaxDef::Specials { trigger, args } => {
            hasher.write_str("specials");
            hasher.write_str(trigger);
            digest_args(hasher, args);
        }
        SyntaxDef::Synthesized => hasher.write_str("synthesized"),
    }
}

/// Adds a list of arguments to the digest: for each argument, its name, its
/// parser's identity (preceded by its length), its mode, its layout, whether
/// it is declared with an opaque wrapper and its scoped packages, as
/// [`digest_body`] does for a body; see [`digest_syntax`].
fn digest_args(hasher: &mut Fnv1a64, args: &[ArgDef]) {
    for arg in args {
        hasher.write_str(arg.name.as_deref().unwrap_or(""));
        let identity = arg.kind.identity();
        hasher.write_u64(identity.len() as u64);
        hasher.write_bytes(&identity);
        hasher.write_str(&format!("{:?}", arg.mode));
        hasher.write_str(&format!("{:?}", arg.layout));
        hasher.write_u64(u64::from(arg.opaque_wrapper));
        for package in &arg.scoped {
            hasher.write_str(package.as_str());
        }
    }
}

/// Adds a body declaration to the digest: its layout, whether it is declared
/// with an opaque wrapper, its parser's identity (or the fact that it has
/// none), its mode and its scoped packages; see [`digest_syntax`].
fn digest_body(hasher: &mut Fnv1a64, body: &BodyDef) {
    hasher.write_str(&format!("{:?}", body.layout));
    hasher.write_u64(u64::from(body.opaque_wrapper));
    match &body.parser {
        None => hasher.write_u64(0),
        Some(parser) => {
            let identity = parser.identity();
            hasher.write_u64(1 + identity.len() as u64);
            hasher.write_bytes(&identity);
        }
    }
    hasher.write_str(&format!("{:?}", body.mode));
    for package in &body.scoped {
        hasher.write_str(package.as_str());
    }
}

/// Builds a registry from `features` alone, with the default inputs
/// ([`RegistryInputs::new`]).
///
/// This function creates a [`RegistryBuilder`], adds each feature in order
/// and calls [`build`](RegistryBuilder::build). Use the builder itself when the
/// program has packages to use, service contributions or workflow fields of
/// its own to give, or inputs to set.
///
/// # Errors
///
/// A [`RegistryBuildError`], exactly as [`RegistryBuilder::build`] reports it.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{
///     BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry,
/// };
///
/// struct Empty;
/// impl FlmFeature for Empty {
///     fn info(&self) -> FeatureInfo { FeatureInfo::new("empty", "Empty") }
///     fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> { Ok(()) }
/// }
///
/// let registry = build_registry([Arc::new(Empty) as Arc<dyn FlmFeature>])?;
/// assert_eq!(registry.features().len(), 1);
/// # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
/// ```
pub fn build_registry(
    features: impl IntoIterator<Item = Arc<dyn FlmFeature>>,
) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    for feature in features {
        builder.feature(feature);
    }
    builder.build(&RegistryInputs::new())
}
