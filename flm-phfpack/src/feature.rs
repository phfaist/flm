//! The feature `phfqit`: it registers one element and one lowering rule per
//! macro, compiles the macros' templates when the registry is built, and
//! replaces each use of a macro by its template, with the delimiters drawn at
//! the size the use asks for.
//!
//! Every macro, built-in or declared in the feature's settings, is a [user
//! definition](Definition) (a macro defined by substitution) held in a
//! [`UserDefinition`]. Its element carries this feature's own expansion hook
//! (`MacroHook`, an [`Expand`] implementation) in place of the definition's.
//! The templates of the feature's macros may write four placeholders whose
//! content the feature computes at each use, the *computed placeholders*:
//! `DelimLeft`, `DelimMiddle` and `DelimRight` (a delimiter at the size the use
//! asks for) and `config` (a named value of the settings).
//!
//! The work is done in two steps:
//!
//! - When the registry is built, the feature's build hook (`Prepare`, a
//!   [`LateInit`] implementation) compiles every macro's templates, as the
//!   `userdefs` feature compiles every user definition's. It then compiles each
//!   text that a computed placeholder refers to (every delimiter the templates
//!   write, at each size, and every named value they read) into a small
//!   template of its own (`Prepared`).
//! - When a use of a macro is expanded, the expansion hook reads the size the
//!   use asks for and instantiates the macro's template with placeholder
//!   bindings (`MacroBindings`) that give each computed placeholder its small
//!   compiled template and pass every other placeholder to the definition's own
//!   bindings ([`DefinitionBindings`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, OnceLock};

use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, LateInit, LateInitError,
};
use flm_core::core::settings::SettingsDecl;
use flm_core::lang::rewrites::userdefs::{
    compile_in_modes, compile_template_with, BadDefinition, CompiledTemplate, Definition,
    DefinitionBindings, DefinitionKind, DefinitionProblem, Expand, ExpandCx,
    ExpandError, ExpandPolicy, Expander, Expansion, PlaceholderBindings, PlaceholderTarget,
    PlaceholderValue, PlaceholderVocabulary, TemplateLanguage, TemplateSpec, UserDefinition,
};
use flm_core::lang::rewrites::userdefs::template::minting_diagnostics;
use flm_core::lang::{BuiltLang, Flm, FlmDiagnostics};
use flm_core::model::names::ElementName;
use flm_core::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_core::techy::core::node::{NodeRef, NodeSlice};
use flm_core::techy::error::{Diagnostic, DiagnosticInfo};
use flm_core::techy::latexlike::Mode;
use flm_core::techy::source::SourceSpan;

use crate::macros::{Macro, CONFIG_PLACEHOLDER};
use crate::settings::{default_values, PhfqitConfig, PhfqitData};
use crate::sizing::{size_of_use, sized_delimiter, text_of, BadSize, Side, Size, SlotKind, SIDES};
use crate::tables::{MAIN_DELIMITED_ARGUMENT, MAIN_DELIMITERS, TABLES};

/// The feature's name, `phfqit`: the name under which a program gives the
/// feature's settings
/// ([`PackageSettings::config`](flm_core::core::settings::PackageSettings::config))
/// and switches the feature off
/// ([`PackageSettings::is_off`](flm_core::core::settings::PackageSettings::is_off)).
/// With the `flm` program, a document writes these settings in its front
/// matter under `flm`, `features`, `phfqit`.
pub const FEATURE: &str = "phfqit";

/// The prefix of the name of every element that the feature `phfqit`
/// ([`Phfqit`]) registers, and so of
/// the name of every lowering rule (derived from the element's name): the
/// package's name, then the feature's, `phfpack.phfqit`. The identifiers of the
/// feature's conditions begin with the same prefix.
pub const PREFIX: &str = "phfpack.phfqit";

/// The feature `phfqit` ([`FlmFeature`]): the built-in macros, the macros
/// declared in its settings, and the named values their templates read. The
/// crate documentation lists the macros and the settings.
///
/// Construct it with [`Phfqit::new`] from a [`PhfqitConfig`]; the default value
/// holds no declared macro and the default named values. The package functions
/// [`package`](crate::package) and [`factory`](crate::factory) construct it.
///
/// Registering the feature returns [`BuildError::Feature`] only when a row of
/// the crate's built-in tables holds an invalid argument code, a fault of the
/// crate that its tests rule out. A declared macro with the name of a command
/// that another feature defines in the same parsing mode makes the registry
/// build fail with the registry's own error for two declarations of one
/// command.
#[derive(Debug, Clone, Default)]
pub struct Phfqit {
    config: PhfqitConfig,
}

impl Phfqit {
    /// Returns the feature built with the configuration `config`, which stays
    /// fixed for the life of every registry the feature is built into.
    pub fn new(config: PhfqitConfig) -> Phfqit {
        Phfqit { config }
    }

    /// Returns the configuration the feature was built with.
    pub fn config(&self) -> &PhfqitConfig {
        &self.config
    }

    /// Returns every macro of the feature, in the order they are registered: the
    /// built-in macros of the crate's tables, except each built-in macro that a
    /// declared macro of the same name replaces, then the declared macros.
    /// Each holds its user definition
    /// ([`Definition`](flm_core::lang::rewrites::userdefs::Definition)) and the
    /// positions of its size arguments.
    ///
    /// # Errors
    ///
    /// A message when a row of the built-in tables holds an invalid argument
    /// code: a fault of the crate's tables, which the crate's tests rule out.
    pub fn macros(&self) -> Result<Vec<Macro>, String> {
        let declared: BTreeSet<&str> =
            self.config.declared.iter().map(|m| m.definition.name.as_str()).collect();
        let mut macros = Vec::new();
        for row in TABLES.iter().flat_map(|table| table.iter()) {
            let m = Macro::from_row(row).map_err(|error| error.to_string())?;
            let replaced = m.definition.kind == DefinitionKind::Macro
                && declared.contains(m.definition.name.as_str());
            if !replaced {
                macros.push(m);
            }
        }
        macros.extend(self.config.declared.iter().cloned());
        Ok(macros)
    }
}

/// Returns the name of the element that the feature `phfqit` registers for the
/// user definition `definition`: `phfpack.phfqit.<kind>.<name>`, where `<kind>` is
/// `macro` or `specials`, with `math.` before the name for a definition
/// available in formulas only (the backtick form's element is
/// `` phfpack.phfqit.specials.math.` ``).
///
/// The name is built as [`element_name_under`] builds the element name of any
/// user definition, with [`PREFIX`] as the prefix.
pub fn element_name(definition: &Definition) -> ElementName {
    element_name_under(PREFIX, definition)
}

/// Returns the declaration of the feature's settings: the configuration read
/// from [`PhfqitData`] and converted to [`PhfqitConfig`]; the four keys merged
/// entry by entry, so that a document that sets one entry keeps the default
/// entries it does not set; and the templates, default values and named values
/// marked as FLM markup.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config_from::<PhfqitData, PhfqitConfig>(
            "The macros declared in addition to the built-in ones (`macro_definitions`), and \
             the named values the macros' templates read (`spaceKets`, `spaceOKets`, `space`). \
             Fixed when the registry is built: it cannot vary per document or per render.",
        )
        .merge_entries("macro_definitions", || Ok(serde_json::Value::Object(serde_json::Map::new())))
        .merge_entries("spaceKets", || Ok(default_values("spaceKets")))
        .merge_entries("spaceOKets", || Ok(default_values("spaceOKets")))
        .merge_entries("space", || Ok(default_values("space")))
        .markup("macro_definitions.*.content")
        .markup("macro_definitions.*.defaults")
        .markup("spaceKets.*")
        .markup("spaceOKets.*")
        .markup("space.*")
}

impl FlmFeature for Phfqit {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Quantum-information notation").docs(
            "Notation macros for quantum information: delimited macros (absolute values, norms, \
             averages, bras and kets, inner products, intervals) whose delimiters are drawn at \
             the size a use asks for; operators, gates and groups; quantity macros (entropies \
             and other measures of information); the backtick form, which writes a group \
             between its own delimiters at a size in formulas; and the macros declared in the \
             feature's settings. Every macro is replaced by its template in the document's \
             tree by the `userdefs` feature's `flm.userdefs.expand-always` stage, for every \
             output, including a recomposed LaTeX output that keeps the other user \
             definitions as LaTeX commands.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        let macros = self
            .macros()
            .map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
        let prepared: Arc<OnceLock<Prepared>> = Arc::new(OnceLock::new());
        let mut definitions = Vec::with_capacity(macros.len());
        for m in macros {
            let definition = m.definition.expand(ExpandPolicy::Always);
            let element = element_name(&definition);
            let value = UserDefinition::new(definition);
            let hook: Arc<dyn Expand> = Arc::new(MacroHook {
                definition: Arc::clone(&value),
                slots: m.slots,
                prepared: Arc::clone(&prepared),
            });
            // The rule covers the element on every backend and schedules the
            // `userdefs` feature's stage that expands the `always`
            // definitions, which makes that feature necessary.
            b.lowering_rule(expand_in_tree_rule_for(element.clone(), value.definition()))?;
            b.element(value.element_def_with(owner.clone(), element, hook))?;
            definitions.push(value);
        }
        b.digest_input(&self.config.digest_bytes())?;
        b.late_init(Arc::new(Prepare {
            definitions,
            values: self.config.values.clone(),
            prepared,
        }))?;
        Ok(())
    }
}

/// The small templates compiled for the computed placeholders when the
/// registry is built: each delimiter a macro's template writes, at each size,
/// and each named value a template reads, in each parsing mode for which a
/// template that refers to it is compiled.
///
/// Each small text is kept in each of those modes where it parses, by the rule
/// the library applies to every text read in several modes
/// ([`compile_in_modes`]). A delimiter at a size command, such as `\bigl (`,
/// parses in formulas and not in running text. A template defined for both
/// modes therefore has its sized delimiters in formulas only, and a use in
/// running text that asks for a size reaches a placeholder that has no value
/// there, which the expansion reports as an Error at the use
/// (`flm.userdefs.bad-placeholder`). A text that parses in none of the modes is
/// reported when the registry is built.
// See decision rationale at [§flm-dd-dr:feat-phfqit-computed-modes]
#[derive(Debug, Default)]
struct Prepared {
    /// The compiled delimiters, by parsing mode, side and delimiter text: one
    /// compiled template per size, in the order of [`Size::all`], `None` at a
    /// size where the delimiter does not parse in that mode.
    delimiters: HashMap<(Mode, Side, String), Vec<Option<CompiledTemplate>>>,
    /// The compiled named values, by parsing mode and name (`spaceKets.Bar`).
    values: HashMap<(Mode, String), CompiledTemplate>,
}

/// The feature's build hook ([`LateInit`]): it compiles every macro's
/// templates, then the small templates of [`Prepared`], and stores the latter
/// in the cell that every macro's expansion hook reads.
///
/// A template or a small text that does not compile, and a template that reads
/// a named value the settings do not hold ([`UnknownValue`]), are written to the
/// registry's build diagnostics; the registry is still built, and a use of a
/// macro whose templates did not compile is an Error at the use.
struct Prepare {
    /// Every macro of the feature, as registered.
    definitions: Vec<Arc<UserDefinition>>,
    /// The named values, keyed as a template refers to them (`spaceKets.Bar`).
    values: std::collections::BTreeMap<String, String>,
    /// The cell shared with every macro's expansion hook, filled once.
    prepared: Arc<OnceLock<Prepared>>,
}

/// The definition name under which the small templates of [`Prepared`] are
/// compiled, which the diagnostics of their compilation show.
const PREPARED_DISPLAY: &str = "phfqit";

impl LateInit for Prepare {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let language = match TemplateLanguage::new(lang) {
            Ok(language) => language,
            Err(errors) => {
                for definition in &self.definitions {
                    let display = definition.definition().display_name();
                    for diagnostic in minting_diagnostics(&display, &errors) {
                        diags.push(diagnostic);
                    }
                }
                return Ok(());
            }
        };
        let mut delimiters: BTreeMap<(Side, String), BTreeSet<ModeKey>> = BTreeMap::new();
        let mut values: BTreeMap<String, BTreeSet<ModeKey>> = BTreeMap::new();
        for definition in &self.definitions {
            let (Ok(reported) | Err(reported)) = definition.compile_with(lang, &language);
            for diagnostic in reported {
                diags.push(diagnostic);
            }
            let Some(compiled) = definition.compiled() else { continue };
            for mode in [Mode::Text, Mode::Math] {
                let Some(variant) = compiled.variant(mode) else { continue };
                let templates = std::iter::once(variant.template())
                    .chain(variant.defaults().map(|(_, template)| template));
                for template in templates {
                    for (id, reference) in template.placeholders() {
                        let PlaceholderTarget::Extra(name) = &reference.target else { continue };
                        let node = template.tree().get(id);
                        let written = node
                            .and_then(|node| node.argument_content_nodes(0))
                            .map(|nodes| text_of(&nodes))
                            .unwrap_or_default();
                        if name == CONFIG_PLACEHOLDER {
                            if self.values.contains_key(&written) {
                                values.entry(written).or_default().insert(ModeKey(mode));
                            } else {
                                let span = node
                                    .map(|node| node.span().clone())
                                    .unwrap_or_else(|| SourceSpan::new(template.source(), 0..0));
                                let condition = UnknownValue {
                                    name: compiled.display_name().to_string(),
                                    written,
                                    known: self.values.keys().cloned().collect(),
                                };
                                diags.push(Diagnostic::error(condition, span));
                            }
                        } else if let Some(side) = SIDES.iter().find(|side| side.placeholder == name) {
                            if written.is_empty() {
                                for (_, delims) in MAIN_DELIMITERS {
                                    let delim = delims[side_index(side.side)].to_string();
                                    delimiters.entry((side.side, delim)).or_default().insert(ModeKey(mode));
                                }
                            } else {
                                delimiters.entry((side.side, written)).or_default().insert(ModeKey(mode));
                            }
                        }
                    }
                }
            }
        }
        let mut prepared = Prepared::default();
        let vocabulary = PlaceholderVocabulary::new(0);
        // Compiles `text` in each of `modes`, kept in the modes where it
        // compiles by the library's rule for a text read in several modes.
        let mut compile = |text: &str, modes: &BTreeSet<ModeKey>, what: &str| {
            let (compiled, reported) =
                compile_in_modes(modes.iter().map(|&ModeKey(mode)| mode), |mode, found| {
                    let spec = TemplateSpec {
                        text,
                        mode,
                        vocabulary: &vocabulary,
                        offset: 0,
                        definition: PREPARED_DISPLAY,
                        label: what,
                        origin_label: what,
                        user_definition: false,
                    };
                    match compile_template_with(&spec, lang, &language) {
                        Ok(template) => {
                            found.extend(template.diagnostics().iter().cloned());
                            Some(template)
                        }
                        Err(errors) => {
                            found.extend(errors);
                            None
                        }
                    }
                });
            for diagnostic in reported {
                diags.push(diagnostic);
            }
            compiled
        };
        for ((side, delim), modes) in delimiters {
            let what = format!("the delimiter `{delim}` of a `phfqit` macro");
            for size in Size::all() {
                for (mode, template) in compile(&sized_delimiter(side, size, &delim), &modes, &what) {
                    let sized = prepared
                        .delimiters
                        .entry((mode, side, delim.clone()))
                        .or_insert_with(|| Size::all().map(|_| None).collect());
                    sized[size.index()] = Some(template);
                }
            }
        }
        for (name, modes) in values {
            let Some(text) = self.values.get(&name) else { continue };
            let what = format!("the value `{name}` of the `phfqit` settings");
            for (mode, template) in compile(text, &modes, &what) {
                prepared.values.insert((mode, name.clone()), template);
            }
        }
        // The hook runs once per registry, so the cell is empty here.
        let _ = self.prepared.set(prepared);
        Ok(())
    }
}

/// A parsing mode with an order (text mode first), used as the key of the sets
/// of texts to compile, so that the compilations always run in the same order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModeKey(Mode);

impl ModeKey {
    fn rank(self) -> u8 {
        match self.0 {
            Mode::Text => 0,
            _ => 1,
        }
    }
}

impl PartialOrd for ModeKey {
    fn partial_cmp(&self, other: &ModeKey) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModeKey {
    fn cmp(&self, other: &ModeKey) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

/// Returns the position of `side` in a row of [`MAIN_DELIMITERS`].
fn side_index(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Middle => 1,
        Side::Right => 2,
    }
}

/// Condition `phfpack.phfqit.unknown-value`: a template of a macro of the
/// feature `phfqit` ([`Phfqit`]) reads, with `#[…]{config}`, a named value (a
/// text held in the feature's settings under a key and an entry name, such as
/// `spaceKets.Bar`) that the feature's settings do not hold.
///
/// The feature's build hook, which runs when the registry is built, reports it
/// as an Error among the registry's build diagnostics
/// ([`FlmRegistry::build_diagnostics`](flm_core::core::registry::FlmRegistry::build_diagnostics)),
/// at the placeholder in the template; each use of the macro is then reported
/// again (`flm.userdefs.bad-placeholder`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownValue {
    /// The macro whose template reads the value, as an author writes it
    /// (`\braket`).
    pub name: String,
    /// The value's name as written between the brackets of `#[…]{config}`.
    pub written: String,
    /// The names of the values the settings hold (`spaceKets.Bar`, …).
    pub known: Vec<String>,
}

impl fmt::Display for UnknownValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`#[{}]{{config}}` in the template of `{}` must refer to one of the values defined \
             in the `phfqit` settings: {}",
            self.written,
            self.name,
            self.known.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
        )
    }
}

impl DiagnosticInfo for UnknownValue {
    const IDENTIFIER: &'static str = "phfpack.phfqit.unknown-value";
}

/// The expansion hook ([`Expand`]) of every macro of the feature: it reads the
/// size a use asks for and instantiates the macro's template with
/// [`MacroBindings`].
///
/// A use is an Error at the use, and is replaced by its source text
/// ([`Expander::recover`]), in three cases: the macro's templates did not
/// compile; the macro is not defined in the use's parsing mode (the condition
/// of [`UserDefinition::variant_for_use`]); the size is not a size command or
/// the star, or is given more than once ([`BadSize`]).
struct MacroHook {
    /// The macro's user definition, with its compiled templates.
    definition: Arc<UserDefinition>,
    /// The macro's size slots: each one's argument index and kind.
    slots: Vec<(usize, SlotKind)>,
    /// The cell the build hook fills with the small compiled templates.
    prepared: Arc<OnceLock<Prepared>>,
}

impl Expand for MacroHook {
    fn policy(&self) -> ExpandPolicy {
        ExpandPolicy::Always
    }

    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        let display = self.definition.definition().display_name();
        let mode = node.parsing_state().mode();
        let found = self.definition.variant_for_use(&node).and_then(|variant| {
            // The late-init hook fills the computed placeholders' values when
            // it compiles the templates, so a compiled template has them.
            let prepared = self.prepared.get().ok_or_else(|| {
                BadDefinition::new(display.clone(), DefinitionProblem::NotCompiled)
            })?;
            Ok((variant, prepared))
        });
        let (variant, prepared) = match found {
            Ok(found) => found,
            Err(condition) => {
                let diagnostic = Diagnostic::error(condition, node.span().clone());
                return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
            }
        };
        let size = match size_of_use(&node, &self.slots) {
            Ok(size) => size,
            Err(problem) => {
                let diagnostic =
                    Diagnostic::error(BadSize { name: display, problem }, node.span().clone());
                return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
            }
        };
        let bindings = MacroBindings {
            base: DefinitionBindings::new(node, variant),
            size,
            main: main_delimiters(&node),
            mode,
            prepared,
        };
        Ok(Expansion::Emit(xp.instantiate(cx, node, variant.template(), &bindings)?))
    }
}

/// Returns the three delimiters (left, middle, right) to write around the
/// use's [main delimited argument](MAIN_DELIMITED_ARGUMENT), chosen by the
/// delimiter that opens that argument ([`MAIN_DELIMITERS`]); `None` when the use
/// has no such argument.
fn main_delimiters(node: &NodeRef<'_, Flm>) -> Option<[&'static str; 3]> {
    let nodes = node.argument_nodes_named(MAIN_DELIMITED_ARGUMENT).ok().flatten()?;
    let open = nodes.iter().find_map(|node| node.group_delimiters().map(|(open, _)| open))?;
    MAIN_DELIMITERS.iter().find(|(written, _)| *written == open).map(|(_, delims)| *delims)
}

/// The placeholder bindings ([`PlaceholderBindings`]) of one use of a macro of
/// the feature: they give each computed placeholder its small compiled
/// template, at the use's size and in the use's parsing mode, and pass every
/// other placeholder to the definition's own bindings.
///
/// A computed placeholder with no compiled template (a delimiter that does not
/// parse at that size in that mode, a named value the settings do not hold, a
/// delimiter placeholder written without a delimiter in a macro that has no
/// main delimited argument) has no value
/// ([`PlaceholderValue::Unresolvable`]), which the expansion reports at the
/// use.
struct MacroBindings<'t> {
    /// The definition's own bindings, for every placeholder that is not a
    /// computed one.
    base: DefinitionBindings<'t>,
    /// The size the use asks for.
    size: Size,
    /// The delimiters of the use's main delimited argument, when it has one.
    main: Option<[&'static str; 3]>,
    /// The use's parsing mode.
    mode: Mode,
    /// The small compiled templates.
    prepared: &'t Prepared,
}

impl PlaceholderBindings for MacroBindings<'_> {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        let PlaceholderTarget::Extra(name) = target else {
            return self.base.value(target, substitution);
        };
        let written = substitution.map(|nodes| text_of(&nodes)).unwrap_or_default();
        if name == CONFIG_PLACEHOLDER {
            return match self.prepared.values.get(&(self.mode, written)) {
                Some(template) => PlaceholderValue::Tree(template.tree()),
                None => PlaceholderValue::Unresolvable,
            };
        }
        let Some(side) = SIDES.iter().find(|side| side.placeholder == name) else {
            return self.base.value(target, substitution);
        };
        let delim = match (written.is_empty(), self.main) {
            (false, _) => written,
            (true, Some(main)) => main[side_index(side.side)].to_string(),
            (true, None) => return PlaceholderValue::Unresolvable,
        };
        match self.prepared.delimiters.get(&(self.mode, side.side, delim)).and_then(|sized| sized[self.size.index()].as_ref()) {
            Some(template) => PlaceholderValue::Tree(template.tree()),
            None => PlaceholderValue::Unresolvable,
        }
    }
}
