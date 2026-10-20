//! The `numbering` feature ([`Numbering`]) and its configuration
//! ([`NumberingConfig`]): what the feature registers (the five service slots,
//! the counter table, the numbering options, the numbering stage), the checks
//! it makes on the declarations when the registry is built, and the
//! compilation of every declared text once the language is built.
//!
//! The [module documentation of the numbering feature](super) describes what
//! the feature provides and lists the checks.

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
    chains_of, digest_text, quoted_list, CounterDecl, ParentSetDecl, Parents,
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

/// Returns the declaration of the feature's settings: the fields of
/// [`NumberingConfig`] (`styles`) and of [`NumberingOptions`] (`counters`) are
/// its settings keys ([`settings`](crate::core::settings)). The texts of a
/// counter style and of a within-joiner are declared as markup.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<NumberingConfig>(
            "The counter styles that replace declared ones: each entry replaces, by its \
            name, the counter style a feature declares. Fixed when the registry is built: it \
            cannot vary per document or per render.",
        )
        .options::<NumberingOptions>(
            Bind::Either,
            "Per counter, the counter it is numbered within and the value it has before it \
            is first advanced. Set on the document; a render request may set it again, and \
            its value then replaces the document's for that render.",
        )
        .merge_entries("styles", || crate::core::settings::named::to_data(&NumberingConfig::default().styles, "name"))
        .markup("styles.*.prefix_words")
        .markup("styles.*.delimiters")
        .markup("styles.*.join")
        .markup("styles.*.number_format")
        .markup("styles.*.sub_levels")
        .markup("counters.*.within.counter.joiner.joined")
        .markup("counters.*.within.counter.joiner.template")
}

/// The configuration of the [`numbering`](super) feature: the counter styles
/// that a host replaces.
///
/// A counter style ([`CounterStyleDecl`]) is declared by the feature that
/// numbers with it. A host that wants other words before a number, another
/// number format or other delimiters gives a replacement: a whole counter
/// style declaration with the same name, which takes the place of the
/// declared one. It is how a host changes the words, the number template or
/// the delimiters of a counter style without writing a feature. A document's
/// numbering options ([`NumberingOptions`](super::NumberingOptions)) change
/// only what a counter is numbered within and how the numbers of those
/// counters are written before its own, and a reference point may give a
/// number template of its own in its number source
/// ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format)).
/// A replacement for a counter style that no feature declares fails the
/// registry build, so a misspelled name is reported once instead of changing
/// nothing.
///
/// The configuration is fixed when the registry is built ([settings, options
/// and configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
/// because every text of a counter style is parsed then. A host passes it as
/// the `numbering` field of [`StdcoreConfig`](crate::stdcore::StdcoreConfig),
/// or in data as the `styles` settings key, a map from a counter style's name
/// to its replacement. An entry in data gives only the fields it changes;
/// every other field takes the value that [`CounterStyleDecl::new`] gives (a
/// decimal number, no word, no delimiters, the default join texts), not the
/// declared counter style's value, so a replacement restates every field it
/// keeps.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::{
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
    /// The replacements, each replacing the counter style declaration of the
    /// same name, in the order they were given.
    ///
    /// An entry in data gives only the fields it changes; every other field
    /// takes the value that [`CounterStyleDecl::new`] gives, not the declared
    /// counter style's value, so a replacement restates every field it keeps.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, CounterStyleDecl>"
    )]
    pub styles: Vec<CounterStyleDecl>,
}

impl NumberingConfig {
    /// Returns the default configuration, which replaces no counter style.
    pub fn new() -> NumberingConfig {
        NumberingConfig::default()
    }

    /// Builder: adds `style`, which replaces the counter style declaration of
    /// the same name.
    pub fn style(mut self, style: CounterStyleDecl) -> NumberingConfig {
        self.styles.push(style);
        self
    }

    /// Returns the bytes this configuration contributes to the [registry
    /// digest](crate::model::digest).
    ///
    /// Every field of every replacement is written out, each text preceded by
    /// its length, so that two configurations that differ in any text or
    /// choice never write the same bytes. The bytes are written field by
    /// field instead of being taken from the type's `Debug` output, which
    /// would change with a renamed field and change every registry digest
    /// without any change in behavior.
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

/// Writes one counter style declaration into `out` for a registry digest,
/// field by field. The citations feature digests its own counter style through
/// this function.
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

/// A counter's place in the hierarchy of counters: the counters it may be
/// numbered within, and whether the parent that applies restarts its count.
#[derive(Clone, Debug, Default)]
pub(super) struct Hierarchy {
    /// The counters the counter may be numbered within, in the declared
    /// order: one, or several, of which the one advanced most recently
    /// applies. Empty for a counter numbered within nothing.
    pub(super) parents: Vec<Name>,
    /// Whether the parent that applies is left out of what the count is kept
    /// under.
    pub(super) no_restart: bool,
}

/// What the numbering feature holds once every declaration is read and every
/// text is compiled: the counters, the compiled counter styles, each counter's
/// place in the hierarchy with its compiled within-joiner and nesting joiner,
/// the parent sets and the registered within-joiners.
///
/// The late-initialization hook ([`Compile`]) writes it once, when the
/// registry is built; the numbering stage reads it.
pub(super) struct Compiled {
    /// Every declared counter, by name.
    pub(super) counters: BTreeMap<Name, CounterDecl>,
    /// Per counter numbered within another, the counters it may be numbered
    /// within, resolved from its declaration. A counter numbered within
    /// nothing has no entry.
    pub(super) hierarchy: BTreeMap<Name, Hierarchy>,
    /// The parent sets that features registered, by name, each with its
    /// counters: the parent sets a document's numbering options may refer to.
    pub(super) parent_sets: BTreeMap<Name, Vec<Name>>,
    /// Every declared counter style, compiled, by name, with the host's
    /// replacements applied. It is held behind one shared pointer because the
    /// counter table of every run carries the whole map
    /// ([`Counters::style`](super::Counters::style)).
    pub(super) styles: Arc<BTreeMap<Name, Arc<CounterStyle>>>,
    /// Per counter, the compiled within-joiner that writes its ancestors'
    /// numbers before its own. A counter numbered within nothing has no entry.
    pub(super) within: BTreeMap<Name, Arc<dyn CompiledWithinJoiner>>,
    /// Per counter (every counter has an entry), the compiled nesting joiner:
    /// what a point of a nested scoped instance writes of the enclosing
    /// instance's number. A counter that declares none has the joiner that
    /// writes nothing.
    pub(super) nesting: BTreeMap<Name, Arc<dyn CompiledWithinJoiner>>,
    /// The within-joiners that features registered, by name: the ones a
    /// document's numbering options may ask for.
    pub(super) joiners: BTreeMap<Name, Arc<dyn WithinJoiner>>,
    /// The template language against which every template of this feature is
    /// compiled, built once when the language was built. The numbering stage
    /// compiles the templates of a document's numbering options and of a
    /// point's own number template against it.
    pub(super) language: TemplateLanguage,
    /// The counters a within template may refer to, in name order: the
    /// vocabulary of every within template.
    pub(super) names: Vec<Name>,
}

/// What one registration of the feature holds between the phases of the
/// registry build: the declarations the finalizer checked, and what the
/// late-initialization hook compiled from them.
///
/// Each registration creates its own value, so that registering the same
/// feature into two registries keeps the two apart.
#[derive(Default)]
pub(super) struct Shared {
    declared: OnceLock<Declared>,
    compiled: OnceLock<Compiled>,
}

impl Shared {
    /// Returns what the numbering stage reads, or `None` before the
    /// late-initialization hook has run.
    pub(super) fn compiled(&self) -> Option<&Compiled> {
        self.compiled.get()
    }
}

/// The checked declarations, as the finalizer leaves them for the
/// late-initialization hook.
struct Declared {
    counters: BTreeMap<Name, CounterDecl>,
    hierarchy: BTreeMap<Name, Hierarchy>,
    parent_sets: BTreeMap<Name, Vec<Name>>,
    styles: BTreeMap<Name, CounterStyleDecl>,
    joiners: BTreeMap<Name, Arc<dyn WithinJoiner>>,
    writers: Vec<Arc<dyn NumberWriter>>,
}

/// The `numbering` feature, an [`FlmFeature`]: counters, counter styles, and
/// the number of every numbered reference point.
///
/// The feature registers five service slots (the counters, the counter
/// styles, the within-joiners, the number writers and the parent sets that
/// other features declare), the counter table [`COUNTERS`], the option type
/// [`NumberingOptions`], the stage [`NUMBER`](super::NUMBER), and a finalizer
/// that checks the declarations. It is constructed from its configuration
/// ([`NumberingConfig`]). The [module documentation](super) describes the
/// feature in full.
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
            "Counters, counter styles and the numbers of reference points: the five service \
             slots into which features declare counters, counter styles, within-joiners, \
             number writers and parent sets, the per-document numbering options, the stage \
             that numbers, in document order, every reference point that has a number \
             source, and the table `flm.counters` it writes.",
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
            "The counters that reference points are numbered by, one value per counter: its \
             name, the counter style its numbers are shown in, and the counter it is numbered \
             within.",
        )?;
        b.service_slot::<CounterStyleDecl>(
            STYLES_SLOT,
            Arity::OrderedMany,
            "The counter styles a number may be shown in, one value per counter style: the \
             number format, the sub-levels, the word before the number, the delimiters and \
             the texts that join several numbers.",
        )?;
        b.service_slot::<WithinJoinerDecl>(
            WITHIN_JOINERS_SLOT,
            Arity::OrderedMany,
            "The within-joiners registered by name, one value per within-joiner: code that \
             writes, before a counter's number, the numbers of the counters it is numbered \
             within.",
        )?;
        b.service_slot::<NumberWriterDecl>(
            NUMBER_WRITERS_SLOT,
            Arity::OrderedMany,
            "The number writers registered by name, one value per number writer: code that \
             turns the counter values to be shown in a counter style into the content a \
             reader sees.",
        )?;
        b.service_slot::<ParentSetDecl>(
            PARENT_SETS_SLOT,
            Arity::OrderedMany,
            "The parent sets, one value per set: a named list of counters within which a \
             counter may be numbered; at each reference point, the counter of the set \
             advanced most recently is the parent.",
        )?;
        b.table(
            COUNTERS,
            "Every reference point the numbering stage read, with its counter, its counter \
             value, the counters it is numbered within and its counter style, or nothing for \
             a point that is not numbered.",
        )?;
        b.stage(Arc::new(NumberStage::new(Arc::clone(&shared))))?;
        b.digest_input(&self.config.digest_bytes())?;
        b.finalizer(Arc::new(Validate { shared, config: self.config.clone() }))?;
        Ok(())
    }
}

/// Returns the [`Numbering`] feature with the default configuration, which
/// replaces no counter style, for a package entry
/// ([`PackageEntry::feature`](crate::core::registry::PackageEntry::feature)).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Numbering::default())
}

/// The feature's finalizer: it reads the five service slots once every
/// feature has registered, checks every declaration against the others,
/// applies the host's replacements of counter styles, and registers the
/// late-initialization hook ([`Compile`]) that compiles the texts once the
/// language is built. Every mistake it finds fails the registry build.
struct Validate {
    shared: Arc<Shared>,
    config: NumberingConfig,
}

impl Validate {
    /// Returns the build error that refuses the build, with `detail` and this
    /// feature's name.
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
                return Err(self.refuse(b, format!("two features declare the counter style `{}`", style.name)));
            }
        }
        for replacement in &self.config.styles {
            if !styles.contains_key(&replacement.name) {
                return Err(self.refuse(
                    b,
                    format!(
                        "the configuration replaces the counter style `{}`, which no feature \
                         declares",
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
                    format!("two features declare the within-joiner `{}`", joiner.name()),
                ));
            }
        }

        let mut writers: Vec<Arc<dyn NumberWriter>> = Vec::new();
        for declared in services.services_named::<NumberWriterDecl>(NUMBER_WRITERS_SLOT) {
            let writer = declared.writer();
            if writers.iter().any(|other| other.name() == writer.name()) {
                return Err(self.refuse(
                    b,
                    format!("two features declare the number writer `{}`", writer.name()),
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
                        "the counter style `{}` is written by the number writer `{name}`, which \
                         no feature registers",
                        style.name
                    ),
                ));
            }
        }

        let mut counters: BTreeMap<Name, CounterDecl> = BTreeMap::new();
        for counter in services.services_named::<CounterDecl>(COUNTERS_SLOT) {
            if counter.name.as_str().starts_with(RESERVED_PREFIX) {
                return Err(self.refuse(b, format!(
                    "a feature declares the counter `{}`; a name beginning with \
                     `{RESERVED_PREFIX}` is the numbering feature's own",
                    counter.name
                )));
            }
            if counters.insert(counter.name.clone(), counter.clone()).is_some() {
                return Err(self.refuse(b, format!("two features declare the counter `{}`", counter.name)));
            }
        }

        let mut parent_sets: BTreeMap<Name, Vec<Name>> = BTreeMap::new();
        for set in services.services_named::<ParentSetDecl>(PARENT_SETS_SLOT) {
            if set.name.as_str().starts_with(RESERVED_PREFIX) {
                return Err(self.refuse(b, format!(
                    "a feature declares the parent set `{}`; a name beginning with \
                     `{RESERVED_PREFIX}` is the numbering feature's own",
                    set.name
                )));
            }
            if let Some(unknown) = set.counters.iter().find(|name| !counters.contains_key(*name)) {
                return Err(self.refuse(b, format!(
                    "the parent set `{}` holds the counter `{unknown}`, which no feature declares",
                    set.name
                )));
            }
            if parent_sets.insert(set.name.clone(), set.counters.clone()).is_some() {
                return Err(self.refuse(b, format!("two features declare the parent set `{}`", set.name)));
            }
        }

        let mut hierarchy: BTreeMap<Name, Hierarchy> = BTreeMap::new();
        for counter in counters.values() {
            if let Some(WithinJoinerSpec::Named(name)) = &counter.nesting {
                if !joiners.contains_key(name) {
                    return Err(self.refuse(
                        b,
                        format!(
                            "the counter `{}` writes its nested instances by the joiner \
                             `{name}`, which no feature registers",
                            counter.name
                        ),
                    ));
                }
            }
            if !styles.contains_key(&counter.default_style) {
                return Err(self.refuse(
                    b,
                    format!(
                        "the counter `{}` is shown in the counter style `{}`, which no feature \
                         declares",
                        counter.name, counter.default_style
                    ),
                ));
            }
            let Some(within) = &counter.within else { continue };
            let parents = within.parents.resolve(&parent_sets).map_err(|error| {
                self.refuse(b, match error {
                    ParentsError::UnknownSet(set) => format!(
                        "the counter `{}` is numbered within the parent set `{set}`, which no \
                         feature registers",
                        counter.name
                    ),
                    ParentsError::Empty => format!(
                        "the counter `{}` is numbered within {}, which holds no counter",
                        counter.name,
                        within.parents.describe()
                    ),
                })
            })?;
            if let Some(unknown) = parents.iter().find(|name| !counters.contains_key(*name)) {
                return Err(self.refuse(
                    b,
                    format!(
                        "the counter `{}` is numbered within `{unknown}`, which no feature declares",
                        counter.name
                    ),
                ));
            }
            if let WithinJoinerSpec::Named(name) = &within.joiner {
                if !joiners.contains_key(name) {
                    return Err(self.refuse(
                        b,
                        format!(
                            "the counter `{}` is numbered within {} by the joiner `{name}`, \
                             which no feature registers",
                            counter.name,
                            within.parents.describe()
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
                self.refuse(
                    b,
                    format!("the counters {} are numbered within each other", quoted_list(&cycle)),
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
            return Err(self.refuse(b, "the feature is registered twice in one registry".into()));
        }
        b.late_init(Arc::new(compile))?;
        Ok(())
    }
}

/// Returns the counters that `name` may be numbered within as declared, for
/// [`chains_of`]: the registry build walks the declared hierarchy.
fn parents_of(hierarchy: &BTreeMap<Name, Hierarchy>, name: &Name) -> Vec<Name> {
    hierarchy.get(name).map(|entry| entry.parents.clone()).unwrap_or_default()
}

/// The feature's late-initialization hook: once the language is built, it
/// compiles every counter style (with the number writer it asks for), every
/// declared within-joiner and every nesting joiner, and checks that each
/// joiner writes only counters of the chains the counter may be numbered
/// within.
///
/// A text that is not valid FLM, or that parses with an error, fails the
/// registry build: a counter style is configuration, and a mistake in it is
/// reported once instead of at every document that shows a number. A text
/// that parses with a warning is kept, and the warning is added to the
/// registry's build diagnostics.
struct Compile {
    shared: Arc<Shared>,
}

impl LateInit for Compile {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let Some(declared) = self.shared.declared.get() else { return Ok(()) };
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "the template language of the numbering feature could not be built: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let mut styles = BTreeMap::new();
        for (name, decl) in &declared.styles {
            let style = decl
                .compile_with(lang, &language, &declared.writers)
                .map_err(|error| LateInitError::new(format!("the counter style `{name}` cannot be used: {error}")))?;
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
            let what = format!("the counter `{name}`");
            let joiner = compile_within(declared_within, &what, &cx).map_err(|error| {
                LateInitError::new(format!(
                    "the counter `{name}` cannot be numbered within {}: {error}",
                    declared_within.parents.describe()
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
                    "the counter `{name}` writes the counter `{named}` before its number, so \
                     `{named}` must be one of its ancestors under every chain of counters it may \
                     be numbered within, and under one of these chains it is not"
                )));
            }
            within.insert(name.clone(), joiner);
        }
        let mut nesting = BTreeMap::new();
        for (name, decl) in &declared.counters {
            let joiner = compile_nesting(decl, &cx).map_err(|error| {
                LateInitError::new(format!(
                    "the joiner with which the counter `{name}` writes the numbers of its \
                     nested instances is not valid: {error}"
                ))
            })?;
            if let Some(named) = nesting_outside(joiner.as_ref(), name, |counter| {
                parents_of(&declared.hierarchy, counter)
            }) {
                return Err(LateInitError::new(format!(
                    "the counter `{name}` writes the counter `{named}` before the number of a \
                     nested instance, so `{named}` must be one of its ancestors under every chain \
                     of counters it may be numbered within, and under one of these chains it is \
                     not"
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

/// Returns the first counter that `joiner`, the within-joiner of the counter
/// `counter`, writes for one of the chains `counter` may be numbered within
/// and that is not in that chain, or `None` when, for every chain, every
/// counter it writes is in the chain.
///
/// The rule applies per chain: `#{_parent}` and the joining text write the
/// last counter of the chain they are given, and so pass for every chain; a
/// within template that refers to a counter by name passes only where that
/// counter is an ancestor under every chain. Every chain is enumerated, and
/// the number of chains multiplies with each counter that has several possible
/// parents; the hierarchies of a registry are small.
///
/// `parents_of` gives the hierarchy in which the chains are walked. A cycle
/// found here returns `None`, because the cycle is reported separately.
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

/// Returns the nesting joiner of `decl`, compiled: the form it declares, or
/// the form that writes nothing where it declares none.
///
/// The nesting joiner is compiled as a within-joiner whose only parent is the
/// counter itself, because the ancestor it writes is the enclosing instance of
/// the same counter; `#{_parent}` and the counter's own name then both refer
/// to that instance.
///
/// # Errors
///
/// The error [`compile_within`] returns for the declared form.
pub(super) fn compile_nesting(
    decl: &CounterDecl,
    cx: &WithinCx<'_>,
) -> Result<Arc<dyn CompiledWithinJoiner>, super::style::StyleError> {
    let spec = decl.nesting.clone().unwrap_or(WithinJoinerSpec::Bare);
    let within = Within { parents: Parents::One(decl.name.clone()), joiner: spec, no_restart: false };
    compile_within(&within, &format!("the nested instances of the counter `{}`", decl.name), cx)
}

/// Returns the first counter that `joiner`, the nesting joiner of `counter`,
/// writes for one of the chains `counter` may be numbered within and that is
/// not an ancestor of a nested point under that chain (the chain followed by
/// the counter itself), or `None` when, for every chain, every counter it
/// writes is such an ancestor.
///
/// `parents_of` gives the hierarchy in which the chains are walked. A cycle
/// found here returns `None`, because the cycle is reported separately.
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

/// A counter style in data is an entry of the map [`NumberingConfig::styles`]
/// under the counter style's name, read over the counter style that
/// [`CounterStyleDecl::new`] returns for that name.
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
