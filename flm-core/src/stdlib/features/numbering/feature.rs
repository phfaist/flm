//! This module defines the `numbering` feature ([`Numbering`]) and its
//! configuration ([`NumberingConfig`]), along with the checks of the
//! declarations made in other features for the numbering feature.
//!
//! The checks run in two steps of the registry build. The finalizer
//! [`Validate`] runs once every feature has registered. It reads the
//! declarations from the five service slots of the numbering feature, and
//! checks that every name used in a declaration is declared. The
//! late-initialization hook [`Compile`] runs once the language is built. It
//! compiles the texts of the counter styles and of the within-joiners, and
//! checks which counters each within-joiner writes. If either step finds a
//! mistake, the registry build fails.
//!
//! The [module documentation of the numbering feature](super) describes the
//! feature and lists the mistakes for which the registry build fails.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use crate::core::settings::SettingsDecl;
use crate::model::names::Name;
use crate::model::options::Bind;
use crate::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, Finalizer, FlmFeature, LateInit, LateInitError,
};
use crate::core::services::Arity;
use crate::lang::{BuiltLang, FlmDiagnostics};

use crate::lang::rewrites::userdefs::template::TemplateLanguage;

use super::decl::{
    chains_of, digest_text, CounterDecl, ParentSetDecl, Parents,
    ParentsError, Within, WithinJoinerSpec, COUNTERS_SLOT, NUMBER_WRITERS_SLOT, PARENT_SETS_SLOT,
    RESERVED_PREFIX, STYLES_SLOT, WITHIN_JOINERS_SLOT,
};
use super::joiner::{
    compile_within, CompiledWithinJoiner, WithinCx, WithinJoiner, WithinJoinerDecl,
};
use super::options::NumberingOptions;
use super::stage::NumberStage;
use super::formats::NumberTemplate;
use super::style::{CounterStyle, CounterStyleDecl};
use super::writer::{NumberWriter, NumberWriterDecl};
use super::table::COUNTERS;

/// The registered name of the numbering feature, `numbering`.
pub const FEATURE: &str = "numbering";

/// Returns the settings declaration of the numbering feature
/// ([`settings`](crate::core::settings)). The settings keys are `styles`, the
/// field of [`NumberingConfig`], and `counters`, the field of
/// [`NumberingOptions`]. The texts of the counter styles and of the
/// within-joiners in these settings are declared as FLM markup.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<NumberingConfig>(
            "Counter styles that are used in place of the counter styles declared with the \
            same names. The fields missing from an entry take their default values, and not \
            the values of the declared counter style. Fixed when the registry is built; it \
            cannot vary per document or per render.",
        )
        .options::<NumberingOptions>(
            Bind::Either,
            "For each counter, the counters within which it is numbered and the value it has \
            before it is first advanced. Set on the document. These options can also be set \
            on a render request; the values set there are used in place of those of the \
            document for that render.",
        )
        .merge_entries("styles", || crate::core::settings::named::to_data(&NumberingConfig::default().styles, "name"))
        .remove_false("styles")
        .markup("styles.*.prefix_words")
        .markup("styles.*.delimiters")
        .markup("styles.*.join")
        .markup("styles.*.number_format")
        .markup("styles.*.sub_levels")
        .markup("counters.*.within.counter.joiner.joined")
        .markup("counters.*.within.counter.joiner.template")
}

/// The configuration of the [`numbering`](super) feature: counter styles to be
/// used in place of declared counter styles.
///
/// A counter style ([`CounterStyleDecl`]) specifies how the numbers of a
/// counter are written: the number format, the word before the number (e.g.,
/// `Eq.~`), the delimiters (e.g., parentheses), and the texts that join
/// several numbers. Counter styles are declared in the features that number
/// constructs, such as the headings feature. With this configuration, a host
/// program can change a declared counter style without writing a feature.
/// Each entry of [`styles`](NumberingConfig::styles) is a complete counter
/// style that is used in place of the declared counter style with the same
/// name. If no
/// counter style with this name is declared in any feature, the registry build
/// fails.
///
/// The configuration is fixed when the registry is built, because the texts
/// of the counter styles are parsed at that time ([settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// The counter styles cannot be changed for a single document. The numbering
/// options of a document ([`NumberingOptions`](super::NumberingOptions)) can
/// only change the counters within which a counter is numbered, how their
/// numbers are written before the number of the counter, and the initial
/// value of the counter.
///
/// A host program passes the configuration as the `numbering` field of
/// [`StdlibConfig`](crate::stdlib::StdlibConfig), or in settings data as
/// the settings key `styles` of the `numbering` feature. In settings data,
/// `styles` is a map from the name of a counter style to its replacement. The
/// fields missing from an entry take the defaults of
/// [`CounterStyleDecl::new`] (a decimal number, no word, no delimiters, the
/// default join texts), and not the values of the declared counter style.
/// Therefore each field of the declared counter style that is to be kept must
/// be repeated in the entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::numbering::{
///     CounterStyleDecl, Delimiters, NumberingConfig, PrefixWords,
/// };
///
/// let mut equations = CounterStyleDecl::new("flm.math.style.equation");
/// equations.delimiters = Delimiters::new("[", "]");
/// equations.prefix_words = PrefixWords::new("Gl.~", "Gln.~");
/// let config = NumberingConfig::new().style(equations);
/// assert_eq!(config.styles.len(), 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct NumberingConfig {
    /// The replacement counter styles. Each entry is used in place of the
    /// counter style declared with the same name.
    ///
    /// In settings data, the fields missing from an entry take the defaults
    /// of [`CounterStyleDecl::new`], and not the values of the declared
    /// counter style.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, CounterStyleDecl>"
    )]
    pub styles: Vec<CounterStyleDecl>,
}

impl NumberingConfig {
    /// Returns the default configuration, which contains no replacement counter
    /// style.
    pub fn new() -> NumberingConfig {
        NumberingConfig::default()
    }

    /// Adds `style` to the replacement counter styles, and returns the
    /// configuration. `style` is used in place of the counter style declared
    /// with the same name.
    pub fn style(mut self, style: CounterStyleDecl) -> NumberingConfig {
        self.styles.push(style);
        self
    }

    /// Returns the bytes that this configuration contributes to the [registry
    /// digest](crate::model::digest).
    ///
    /// The bytes contain every field of every replacement counter style.
    /// Therefore two configurations that differ in any field give different
    /// bytes.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.styles.len() as u64).to_le_bytes());
        for style in &self.styles {
            digest_style(&mut out, style);
        }
        out
    }
}

impl Hash for NumberingConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// Writes the counter style declaration `style` into `out`, as part of a
/// registry digest. Every field is written, and each text is preceded by its
/// length. The citations feature also uses this function for the counter
/// style of its own configuration.
pub(crate) fn digest_style(out: &mut Vec<u8>, style: &CounterStyleDecl) {
    digest_text(out, style.name.as_str());
    digest_text(out, style.number_format.text());
    out.push(u8::from(matches!(
        style.number_format,
        NumberTemplate::Initial(_)
    )));
    out.extend_from_slice(&(style.sub_levels.len() as u64).to_le_bytes());
    for level in &style.sub_levels {
        digest_text(out, &level.separator);
        digest_text(out, level.number_format.text());
        out.push(u8::from(matches!(
            level.number_format,
            NumberTemplate::Initial(_)
        )));
    }
    digest_text(out, &style.prefix_words.base.one);
    digest_text(out, &style.prefix_words.base.other);
    out.extend_from_slice(&(style.prefix_words.variants.len() as u64).to_le_bytes());
    for (name, words) in &style.prefix_words.variants {
        digest_text(out, name);
        digest_text(out, &words.one);
        digest_text(out, &words.other);
    }
    digest_text(out, &style.delimiters.open);
    digest_text(out, &style.delimiters.close);
    style.join.digest_into(out);
    out.push(u8::from(style.name_in_link));
    out.push(u8::from(style.repeat_parents_in_range));
    out.push(u8::from(style.ranges));
    out.push(u8::from(style.number_writer.is_some()));
    digest_text(out, style.number_writer.as_ref().map_or("", Name::as_str));
}

/// The place of a counter in the hierarchy of counters: the counters within
/// which the counter may be numbered, and whether its parent restarts its
/// count.
#[derive(Clone, Debug, Default)]
pub(super) struct Hierarchy {
    /// The counters within which the counter may be numbered, in the declared
    /// order. If the list contains several counters, the parent at a
    /// reference point is the one advanced most recently. The list is empty
    /// for a counter that is numbered within nothing.
    pub(super) parents: Vec<Name>,
    /// If `true`, the parent does not restart the count of the counter (see
    /// [`Within::no_restart`]).
    pub(super) no_restart: bool,
}

/// The declarations of the numbering feature, checked and compiled when the
/// registry is built.
///
/// The late-initialization hook [`Compile`] creates this value once, and the
/// numbering stage reads it in every run.
pub(super) struct Compiled {
    /// The declared counters, by name.
    pub(super) counters: BTreeMap<Name, CounterDecl>,
    /// The place of each counter in the declared hierarchy, by counter name. A
    /// counter that is numbered within nothing has no entry.
    pub(super) hierarchy: BTreeMap<Name, Hierarchy>,
    /// The declared parent sets, by name, each with its counters. These
    /// parent sets can be used in the numbering options of a document.
    pub(super) parent_sets: BTreeMap<Name, Vec<Name>>,
    /// The compiled counter styles, by name, with the replacements of
    /// [`NumberingConfig`] applied. The map is shared through an `Arc` because
    /// the counter table of every run contains it
    /// ([`Counters::style`](super::Counters::style)).
    pub(super) styles: Arc<BTreeMap<Name, Arc<CounterStyle>>>,
    /// The compiled within-joiner of each counter, by counter name. The
    /// within-joiner writes the numbers of the ancestors of the counter before
    /// the number of the counter. A counter that is numbered within nothing
    /// has no entry.
    pub(super) within: BTreeMap<Name, Arc<dyn CompiledWithinJoiner>>,
    /// The compiled nesting joiner of each counter, by counter name. Every
    /// counter has an entry. The nesting joiner writes the number of the
    /// enclosing instance before the number of a reference point in a nested
    /// scoped instance of the counter. For a counter declared without a
    /// nesting joiner, the entry writes nothing.
    pub(super) nesting: BTreeMap<Name, Arc<dyn CompiledWithinJoiner>>,
    /// The within-joiners declared by name in the registry, by name. These
    /// within-joiners can be selected by name in the numbering options of a
    /// document.
    pub(super) joiners: BTreeMap<Name, Arc<dyn WithinJoiner>>,
    /// The template language with which the templates of the counter styles
    /// and of the within-joiners are compiled. The numbering stage also
    /// compiles with it the templates given in the numbering options of a
    /// document and the number template of a reference point.
    pub(super) language: TemplateLanguage,
    /// The names of all declared counters, in name order. These counters can
    /// be referred to in a within template.
    pub(super) names: Vec<Name>,
}

/// The state of one registration of the numbering feature, shared by the
/// finalizer [`Validate`], the late-initialization hook [`Compile`] and the
/// numbering stage. The finalizer stores the checked declarations, and the
/// hook stores the compiled declarations ([`Compiled`]).
///
/// Each call of [`register`](FlmFeature::register) creates its own `Shared`
/// value. Therefore one [`Numbering`] value registered into two registries
/// has a separate state in each registry.
#[derive(Default)]
pub(super) struct Shared {
    declared: OnceLock<Declared>,
    compiled: OnceLock<Compiled>,
}

impl Shared {
    /// Returns the compiled declarations, or `None` before the
    /// late-initialization hook has run.
    pub(super) fn compiled(&self) -> Option<&Compiled> {
        self.compiled.get()
    }
}

/// The declarations of the numbering feature, after the checks of the
/// finalizer [`Validate`]. The late-initialization hook [`Compile`] compiles
/// them.
struct Declared {
    counters: BTreeMap<Name, CounterDecl>,
    hierarchy: BTreeMap<Name, Hierarchy>,
    parent_sets: BTreeMap<Name, Vec<Name>>,
    styles: BTreeMap<Name, CounterStyleDecl>,
    joiners: BTreeMap<Name, Arc<dyn WithinJoiner>>,
    writers: Vec<Arc<dyn NumberWriter>>,
}

/// The `numbering` feature, which numbers the reference points of a document,
/// such as headings, equations and theorems, with counters.
///
/// The counters, the counter styles, the within-joiners, the number writers
/// and the parent sets are declared in other features, in the five service
/// slots of this feature. The feature checks these declarations when the
/// registry is built, and the registry build fails if a declaration contains
/// a mistake. For each document, the numbering stage
/// [`NUMBER`](super::NUMBER) numbers the reference points and writes their
/// numbers to the counter table [`COUNTERS`]. The numbering of a document can
/// be changed with the option type [`NumberingOptions`].
///
/// Create the feature with [`Numbering::new`] and a [`NumberingConfig`], or
/// with [`feature()`] for the default configuration. The [module
/// documentation](super) describes the feature in full.
#[derive(Debug, Clone, Default)]
pub struct Numbering {
    config: NumberingConfig,
}

impl Numbering {
    /// Returns the feature, configured with `config`.
    pub fn new(config: NumberingConfig) -> Numbering {
        Numbering { config }
    }
}

impl FlmFeature for Numbering {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Numbering").docs(
            "The numbers of headings, equations, theorems and other reference points of a \
             document. Each reference point that has a number source is numbered with a \
             counter, in document order, and its number is written in a counter style. \
             Counters, counter styles, within-joiners, number writers and parent sets are \
             declared in other features, in the five service slots of this feature. The \
             numbering of a document can be changed with its numbering options. The numbers \
             are written to the table `flm.counters`.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let shared = Arc::new(Shared::default());
        b.service_slot::<CounterDecl>(
            COUNTERS_SLOT,
            Arity::OrderedMany,
            "The counters with which reference points are numbered, one value per counter. \
             A value contains the name of the counter, the default counter style of its \
             numbers, and the counters within which it is numbered.",
        )?;
        b.service_slot::<CounterStyleDecl>(
            STYLES_SLOT,
            Arity::OrderedMany,
            "The counter styles in which numbers are written, one value per counter style. \
             A value contains the number format, the sub-levels, the word before the number, \
             the delimiters and the texts that join several numbers.",
        )?;
        b.service_slot::<WithinJoinerDecl>(
            WITHIN_JOINERS_SLOT,
            Arity::OrderedMany,
            "The within-joiners that can be selected by name, one value per within-joiner. \
             A within-joiner is code that writes the numbers of the counters within which a \
             counter is numbered, before the number of the counter.",
        )?;
        b.service_slot::<NumberWriterDecl>(
            NUMBER_WRITERS_SLOT,
            Arity::OrderedMany,
            "The number writers that can be selected by name in a counter style, one value \
             per number writer. A number writer is code that writes counter values as \
             content, for the counter styles in which it is selected.",
        )?;
        b.service_slot::<ParentSetDecl>(
            PARENT_SETS_SLOT,
            Arity::OrderedMany,
            "The parent sets, one value per parent set. A parent set is a named list of \
             counters within which a counter can be numbered. At each reference point, the \
             counter of the set that was advanced most recently is the parent.",
        )?;
        b.table(
            COUNTERS,
            "The numbers of the reference points of a document. For each numbered reference \
             point, the table contains its counter, its counter value, the values of the \
             counters within which it is numbered, and its counter style.",
        )?;
        b.stage(Arc::new(NumberStage::new(Arc::clone(&shared))))?;
        b.digest_input(&self.config.digest_bytes())?;
        b.finalizer(Arc::new(Validate { shared, config: self.config.clone() }))?;
        Ok(())
    }
}

/// Returns the [`Numbering`] feature with the default configuration, which
/// contains no replacement counter style. You can pass the result to
/// [`PackageEntry::feature`](crate::core::registry::PackageEntry::feature) to
/// add the feature to a package.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Numbering::default())
}

/// The finalizer of the numbering feature, which runs once every feature has
/// registered.
///
/// The finalizer reads the declarations from the five service slots of the
/// feature, and applies the replacement counter styles of
/// [`NumberingConfig`]. It checks that no name is declared twice, that no
/// counter or parent set has a name that begins with `_`
/// ([`RESERVED_PREFIX`]), that every name used in a declaration is declared,
/// and that no counters are numbered within each other. At the first mistake,
/// the finalizer returns an error, and the registry build fails. Finally, the
/// finalizer stores the
/// checked declarations in [`Shared`] and registers the late-initialization
/// hook [`Compile`].
struct Validate {
    shared: Arc<Shared>,
    config: NumberingConfig,
}

impl Validate {
    /// Returns the error with the message `detail`, attributed to the
    /// numbering feature, with which the registry build fails.
    fn refuse(&self, b: &FeatureBuilder<'_>, detail: String) -> BuildError {
        BuildError::Feature { feature: b.feature().clone(), detail }
    }
}

impl Finalizer for Validate {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let services = b.services().expect("a finalizer runs after the slots are assembled");

        let mut styles: BTreeMap<Name, CounterStyleDecl> = BTreeMap::new();
        for style in services.services_named::<CounterStyleDecl>(STYLES_SLOT) {
            if styles.insert(style.name.clone(), style.clone()).is_some() {
                return Err(self.refuse(
                    b,
                    format!("Duplicate declaration of counter style ‘{}’", style.name),
                ));
            }
        }
        for replacement in &self.config.styles {
            if !styles.contains_key(&replacement.name) {
                return Err(self.refuse(
                    b,
                    format!(
                        "Invalid replacement for undeclared counter style ‘{}’",
                        replacement.name
                    ),
                ));
            }
            styles.insert(replacement.name.clone(), replacement.clone());
        }

        let mut joiners: BTreeMap<Name, Arc<dyn WithinJoiner>> = BTreeMap::new();
        for declared in services.services_named::<WithinJoinerDecl>(WITHIN_JOINERS_SLOT) {
            let joiner = declared.joiner();
            if joiners.insert(joiner.name().clone(), Arc::clone(joiner)).is_some() {
                return Err(self.refuse(
                    b,
                    format!("Duplicate declaration of within-joiner ‘{}’", joiner.name()),
                ));
            }
        }

        let mut writers: Vec<Arc<dyn NumberWriter>> = Vec::new();
        for declared in services.services_named::<NumberWriterDecl>(NUMBER_WRITERS_SLOT) {
            let writer = declared.writer();
            if writers.iter().any(|other| other.name() == writer.name()) {
                return Err(self.refuse(
                    b,
                    format!("Duplicate declaration of number writer ‘{}’", writer.name()),
                ));
            }
            writers.push(Arc::clone(writer));
        }
        for style in styles.values() {
            let Some(name) = &style.number_writer else { continue };
            if !writers.iter().any(|writer| writer.name() == name) {
                return Err(self.refuse(
                    b,
                    format!(
                        "Unknown number writer ‘{name}’ of counter style ‘{}’",
                        style.name
                    ),
                ));
            }
        }

        let mut counters: BTreeMap<Name, CounterDecl> = BTreeMap::new();
        for counter in services.services_named::<CounterDecl>(COUNTERS_SLOT) {
            if counter.name.as_str().starts_with(RESERVED_PREFIX) {
                return Err(self.refuse(b, format!(
                    "Invalid counter name ‘{}’: counter names may not begin with \
                     ‘{RESERVED_PREFIX}’",
                    counter.name
                )));
            }
            if counters.insert(counter.name.clone(), counter.clone()).is_some() {
                return Err(self.refuse(
                    b,
                    format!("Duplicate declaration of counter ‘{}’", counter.name),
                ));
            }
        }

        let mut parent_sets: BTreeMap<Name, Vec<Name>> = BTreeMap::new();
        for set in services.services_named::<ParentSetDecl>(PARENT_SETS_SLOT) {
            if set.name.as_str().starts_with(RESERVED_PREFIX) {
                return Err(self.refuse(b, format!(
                    "Invalid parent set name ‘{}’: parent set names may not begin \
                     with ‘{RESERVED_PREFIX}’",
                    set.name
                )));
            }
            if let Some(unknown) = set.counters.iter().find(|name| !counters.contains_key(*name)) {
                return Err(self.refuse(b, format!(
                    "Unknown counter ‘{unknown}’ in parent set ‘{}’",
                    set.name
                )));
            }
            if parent_sets.insert(set.name.clone(), set.counters.clone()).is_some() {
                return Err(self.refuse(
                    b,
                    format!("Duplicate declaration of parent set ‘{}’", set.name),
                ));
            }
        }

        let mut hierarchy: BTreeMap<Name, Hierarchy> = BTreeMap::new();
        for counter in counters.values() {
            if let Some(WithinJoinerSpec::Named(name)) = &counter.nesting {
                if !joiners.contains_key(name) {
                    return Err(self.refuse(
                        b,
                        format!(
                            "Unknown within-joiner ‘{name}’ for the nesting joiner of \
                             counter ‘{}’",
                            counter.name
                        ),
                    ));
                }
            }
            if !styles.contains_key(&counter.default_style) {
                return Err(self.refuse(
                    b,
                    format!(
                        "Unknown default counter style ‘{}’ of counter ‘{}’",
                        counter.default_style, counter.name
                    ),
                ));
            }
            let Some(within) = &counter.within else { continue };
            let parents = within.parents.resolve(&parent_sets).map_err(|error| {
                self.refuse(b, match (error, &within.parents) {
                    (ParentsError::UnknownSet(set), _) => format!(
                        "Unknown parent set ‘{set}’ for counter ‘{}’",
                        counter.name
                    ),
                    (ParentsError::Empty, Parents::Set(set)) => format!(
                        "Empty parent set ‘{set}’ for counter ‘{}’",
                        counter.name
                    ),
                    (ParentsError::Empty, _) => format!(
                        "Empty list of parent counters for counter ‘{}’",
                        counter.name
                    ),
                })
            })?;
            if let Some(unknown) = parents.iter().find(|name| !counters.contains_key(*name)) {
                return Err(self.refuse(
                    b,
                    format!(
                        "Unknown parent counter ‘{unknown}’ for counter ‘{}’",
                        counter.name
                    ),
                ));
            }
            if let WithinJoinerSpec::Named(name) = &within.joiner {
                if !joiners.contains_key(name) {
                    return Err(self.refuse(
                        b,
                        format!(
                            "Unknown within-joiner ‘{name}’ for counter ‘{}’",
                            counter.name
                        ),
                    ));
                }
            }
            hierarchy.insert(
                counter.name.clone(),
                Hierarchy { parents, no_restart: within.no_restart },
            );
        }

        for counter in counters.keys() {
            chains_of(counter, &|name| parents_of(&hierarchy, name)).map_err(|cycle| {
                let quoted: Vec<String> =
                    cycle.iter().map(|name| format!("‘{name}’")).collect();
                self.refuse(
                    b,
                    format!(
                        "Circular hierarchy of counters (counters concerned: {})",
                        quoted.join(", ")
                    ),
                )
            })?;
        }

        // Which counters a joiner writes is known only once it is compiled — a
        // template's names are read off the compiled template — so the check
        // that a counter names only counters it may be numbered within is made
        // by the late-initialization hook, which also fails the build.
        let declared = Declared { counters, hierarchy, parent_sets, styles, joiners, writers };
        let compile = Compile { shared: Arc::clone(&self.shared) };
        if self.shared.declared.set(declared).is_err() {
            return Err(self.refuse(
                b,
                "Finalizer of feature ‘numbering’ called twice for one registration"
                    .into(),
            ));
        }
        b.late_init(Arc::new(compile))?;
        Ok(())
    }
}

/// Returns the counters within which the counter `name` may be numbered in the
/// declared hierarchy `hierarchy`. Returns an empty list for a counter that
/// `hierarchy` does not contain. During the registry build, this function is
/// passed to [`chains_of`].
fn parents_of(hierarchy: &BTreeMap<Name, Hierarchy>, name: &Name) -> Vec<Name> {
    hierarchy.get(name).map(|entry| entry.parents.clone()).unwrap_or_default()
}

/// The late-initialization hook of the numbering feature, which runs once the
/// language that documents are parsed with is built.
///
/// The hook compiles the counter styles, each with the number writer selected
/// in it. It also compiles the within-joiner and the nesting joiner of each
/// counter. It then checks that each within-joiner and each nesting joiner
/// writes only the numbers of ancestors of the counter. A counter with several
/// possible parents has several possible chains of ancestors ([`chains_of`]),
/// and a counter that a joiner writes must be in each of these chains (see
/// [`written_outside`] and [`nesting_outside`]). Finally, the hook stores the
/// compiled declarations ([`Compiled`]) in [`Shared`].
///
/// If a text is not valid FLM or does not compile, the hook returns an error,
/// and the registry build fails. A counter style is configuration, so a mistake in it is reported
/// once, when the registry is built, and not in every document. The warnings
/// reported while the counter styles are compiled are added to the build
/// diagnostics of the registry.
struct Compile {
    shared: Arc<Shared>,
}

impl LateInit for Compile {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let Some(declared) = self.shared.declared.get() else { return Ok(()) };
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "Failed to build the template language of the numbering feature: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let mut styles = BTreeMap::new();
        for (name, decl) in &declared.styles {
            let style = decl
                .compile_with(lang, &language, &declared.writers)
                .map_err(|error| {
                    LateInitError::new(format!("Invalid counter style ‘{name}’: {error}"))
                })?;
            for warning in style.warnings() {
                diags.push(warning.clone());
            }
            styles.insert(name.clone(), Arc::new(style));
        }
        let names: Vec<Name> = declared.counters.keys().cloned().collect();
        let cx = WithinCx::new(lang, &language, &names, &declared.joiners);
        let mut within = BTreeMap::new();
        for (name, decl) in &declared.counters {
            let Some(declared_within) = &decl.within else { continue };
            let joiner = compile_within(declared_within, name.as_str(), &cx).map_err(|error| {
                LateInitError::new(format!(
                    "Invalid within-joiner of counter ‘{name}’: {error}"
                ))
            })?;
            // Under every chain the counter may have, the joiner may write only
            // counters of that chain: the check is made here, where the joiner
            // can be asked, and it fails the build as every other mistake in a
            // declaration does.
            if let Some(named) = written_outside(joiner.as_ref(), name, |counter| {
                parents_of(&declared.hierarchy, counter)
            }) {
                return Err(LateInitError::new(format!(
                    "Invalid reference to counter ‘{named}’ in the within-joiner of counter \
                     ‘{name}’: ‘{name}’ is not always numbered within ‘{named}’"
                )));
            }
            within.insert(name.clone(), joiner);
        }
        let mut nesting = BTreeMap::new();
        for (name, decl) in &declared.counters {
            let joiner = compile_nesting(decl, &cx).map_err(|error| {
                LateInitError::new(format!(
                    "Invalid nesting joiner of counter ‘{name}’: {error}"
                ))
            })?;
            if let Some(named) = nesting_outside(joiner.as_ref(), name, |counter| {
                parents_of(&declared.hierarchy, counter)
            }) {
                return Err(LateInitError::new(format!(
                    "Invalid reference to counter ‘{named}’ in the nesting joiner of counter \
                     ‘{name}’: ‘{name}’ is not always numbered within ‘{named}’"
                )));
            }
            nesting.insert(name.clone(), joiner);
        }
        // The hook is registered once per registration of the feature, the
        // finalizer refusing a second registration into one registry, so the
        // cell is empty here. Were it ever called twice, what the stage reads
        // stays the first whole compilation rather than becoming half of two.
        let _ = self.shared.compiled.set(Compiled {
            counters: declared.counters.clone(),
            hierarchy: declared.hierarchy.clone(),
            parent_sets: declared.parent_sets.clone(),
            styles: Arc::new(styles),
            within,
            nesting,
            joiners: declared.joiners.clone(),
            language,
            names,
        });
        Ok(())
    }
}

/// Checks the counters that the within-joiner `joiner` of the counter
/// `counter` writes. Returns the first written counter that is missing from
/// one of the chains of ancestors of `counter`, or `None` if every written
/// counter is in every chain.
///
/// The chains of ancestors are computed by [`chains_of`], with `parents_of`
/// as the hierarchy of counters. A counter with several possible parents has
/// several chains, and `joiner` is checked with each chain. For instance, the
/// joining text is written after the number of the parent of the chain, and
/// `#{_parent}` is replaced by this number, so both are valid for every chain.
/// A within template that contains the name of a counter, as in
/// `#{flm.headings.section}`, is valid only if this counter is in every
/// chain.
///
/// If the hierarchy contains a cycle, the function returns `None`, because
/// the cycle is reported separately.
pub(super) fn written_outside(
    joiner: &dyn CompiledWithinJoiner,
    counter: &Name,
    parents_of: impl Fn(&Name) -> Vec<Name>,
) -> Option<Name> {
    let chains = chains_of(counter, &parents_of).ok()?;
    chains.iter().find_map(|chain| {
        joiner.counters_written(chain).into_iter().find(|named| !chain.contains(named))
    })
}

/// Compiles the nesting joiner of the counter declared by `decl`. For a counter
/// declared without a nesting joiner, the function compiles the bare form
/// ([`WithinJoinerSpec::Bare`]), which writes nothing.
///
/// The nesting joiner is compiled as a within-joiner whose only parent is the
/// counter itself, because the number that it writes is the number of the
/// enclosing instance of the same counter. In the nesting joiner, `#{_parent}`
/// and the name of the counter therefore both refer to the enclosing
/// instance.
///
/// # Errors
///
/// Returns the error of [`compile_within`] if the nesting joiner cannot be
/// compiled.
pub(super) fn compile_nesting(
    decl: &CounterDecl,
    cx: &WithinCx<'_>,
) -> Result<Arc<dyn CompiledWithinJoiner>, super::style::StyleError> {
    let spec = decl.nesting.clone().unwrap_or(WithinJoinerSpec::Bare);
    let within = Within { parents: Parents::One(decl.name.clone()), joiner: spec, no_restart: false };
    compile_within(&within, decl.name.as_str(), cx)
}

/// Checks the counters that the nesting joiner `joiner` of the counter
/// `counter` writes. Returns the first written counter that is missing from
/// one of the chains of ancestors of a reference point in a nested instance
/// of `counter`, or `None` if every written counter is in every chain.
///
/// For a reference point in a nested instance, each chain of ancestors is a
/// chain of ancestors of `counter` followed by `counter` itself, for the
/// enclosing instance. The chains of ancestors of `counter` are computed by
/// [`chains_of`], with `parents_of` as the hierarchy of counters.
///
/// If the hierarchy contains a cycle, the function returns `None`, because
/// the cycle is reported separately.
pub(super) fn nesting_outside(
    joiner: &dyn CompiledWithinJoiner,
    counter: &Name,
    parents_of: impl Fn(&Name) -> Vec<Name>,
) -> Option<Name> {
    let chains = chains_of(counter, &parents_of).ok()?;
    chains.into_iter().find_map(|mut chain| {
        chain.push(counter.clone());
        joiner.counters_written(&chain).into_iter().find(|named| !chain.contains(named))
    })
}

/// Reads a counter style from an entry of the settings key `styles`
/// ([`NumberingConfig::styles`]). In settings data, `styles` is a map from the
/// name of a counter style to its fields. The entry is read over the result of
/// [`CounterStyleDecl::new`] for its name, so the fields missing from the entry
/// keep the defaults of [`CounterStyleDecl::new`].
impl crate::core::settings::named::NamedEntry for CounterStyleDecl {
    fn read_entry(name: &str, data: serde_json::Value) -> Result<CounterStyleDecl, String> {
        crate::core::settings::named::read_over(&CounterStyleDecl::new(name), "name", name, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_parents_are_the_ones_the_hierarchy_holds() {
        let mut hierarchy = BTreeMap::new();
        hierarchy.insert(
            Name::new("equation"),
            Hierarchy { parents: vec![Name::new("section"), Name::new("subsection")], no_restart: false },
        );
        assert_eq!(
            parents_of(&hierarchy, &Name::new("equation")),
            [Name::new("section"), Name::new("subsection")]
        );
        assert!(parents_of(&hierarchy, &Name::new("section")).is_empty());
    }

    #[test]
    fn two_configurations_that_replace_different_texts_digest_differently() {
        let one = NumberingConfig::new().style(CounterStyleDecl::new("a.style"));
        let other = NumberingConfig::new().style(CounterStyleDecl::new("b.style"));
        assert_ne!(one.digest_bytes(), other.digest_bytes());
        assert_eq!(NumberingConfig::new().digest_bytes(), NumberingConfig::new().digest_bytes());
    }
}
