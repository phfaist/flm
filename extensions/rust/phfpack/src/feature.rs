//! The feature `phfqit`: it registers one element and one lowering rule per
//! macro, and replaces each use of a macro by its template, with the
//! delimiters drawn at the size the use asks for.
//!
//! Every macro, built-in or declared in the feature's settings, is a [user
//! definition](flm_ext::guest::lang::rewrites::userdefs::Definition) (a macro
//! defined by substitution) held in a
//! [`UserDefinition`]. Its element carries this feature's own expansion hook
//! (`MacroHook`, an [`Expand`] implementation) in place of the definition's.
//! The templates of the feature's macros may write four placeholders whose
//! content the feature computes at each use, the *computed placeholders*:
//! `DelimLeft`, `DelimMiddle` and `DelimRight` (a delimiter at the size the use
//! asks for) and `config` (a named value of the settings).
//!
//! The work is done in two steps:
//!
//! - When the registry is built, [`register`] declares each macro's element and
//!   lowering rule. The adapter of the extension interface compiles every
//!   macro's templates once the language is built, as the `userdefs` feature
//!   compiles every user definition's.
//! - When a use of a macro is expanded, the expansion hook reads the size the
//!   use asks for, lists the computed placeholders that the macro's templates
//!   write, and compiles each text they stand for (the delimiter at the use's
//!   size, the named value) into a small template of its own, in the use's
//!   parsing mode. It then instantiates the macro's template with the
//!   definition's own bindings ([`DefinitionBindings`]), over which the small
//!   templates are laid ([`ExtraValues`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::lang::BuiltLang;
use flm_ext::guest::lang::mint::Mode;
use flm_ext::guest::lang::rewrites::userdefs::{
    CompiledTemplate, DefinitionBindings, DefinitionKind, Expand, ExpandCx, ExpandPolicy, Expander,
    Expansion, ExtraValues, PlaceholderTarget, PlaceholderVocabulary, TemplateLanguage, TemplateSpec, UserDefinition,
    compile_template_with,
};
use flm_ext::guest::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::Node;
use serde_json::{Value, json};

use crate::macros::{CONFIG_PLACEHOLDER, Macro, display_name};
use crate::settings::{self, PhfqitConfig, PhfqitData, default_values};
use crate::sizing::{BAD_SIZE, SIDES, Side, SlotKind, size_of_use, sized_delimiter};
use crate::tables::{MAIN_DELIMITED_ARGUMENT, MAIN_DELIMITERS, TABLES};

/// The feature's name, `phfqit`: the name under which a document gives the
/// feature's settings (in its front matter under `flm`, `features`, `phfqit`)
/// and switches the feature off.
pub const FEATURE: &str = "phfqit";

/// The prefix of the name of every element that the feature `phfqit`
/// registers, and so of the name of every lowering rule (derived from the
/// element's name): the package's name, then the feature's, `phfpack.phfqit`.
/// The identifiers of the feature's diagnostics begin with the same prefix.
pub const PREFIX: &str = "phfpack.phfqit";

/// The identifier of the Error reported at a use of a macro whose template
/// reads, with `#[…]{config}`, a named value that the feature's settings do
/// not hold: `phfpack.phfqit.unknown-value`. The use is replaced by its source
/// text.
pub const UNKNOWN_VALUE: &str = "phfpack.phfqit.unknown-value";

/// The identifier of the library's condition for a use of a user definition
/// that has no valid variant in the use's parsing mode, which the expansion
/// hook reports under that identifier.
const BAD_DEFINITION: &str = "flm.userdefs.bad-definition";

/// The kind of the error that `Node::argument_nodes_named` answers for a name
/// that the node's macro does not declare.
const NO_SUCH_ARGUMENT: &str = "access";

/// The kind of the error that `compile_template_with` answers for a text that
/// does not compile.
const COMPILE_TEMPLATE: &str = "compile_template";

/// The definition name under which the small templates of the computed
/// placeholders are compiled, which the diagnostics of their compilation show.
const SMALL_TEMPLATES: &str = "phfqit";

/// Returns the declaration of the feature's settings, as data: the
/// configuration with its JSON schema ([`settings::schema`]); the four keys
/// merged entry by entry, so that a document that sets one entry keeps the
/// default entries it does not set; and the templates, default values and named
/// values marked as FLM markup.
pub fn settings() -> Value {
    json!({
        "config": {
            "schema": settings::schema(),
            "docs": "The macros declared in addition to the built-in ones (`macro_definitions`), and the named \
                     values the macros' templates read (`spaceKets`, `spaceOKets`, `space`). Fixed when the \
                     registry is built: it cannot vary per document or per render.",
        },
        "merge_entries": {
            "macro_definitions": {},
            "spaceKets": default_values("spaceKets"),
            "spaceOKets": default_values("spaceOKets"),
            "space": default_values("space"),
        },
        "markup": [
            "macro_definitions.*.content",
            "macro_definitions.*.defaults",
            "spaceKets.*",
            "spaceOKets.*",
            "space.*",
        ],
    })
}

/// Registers the feature `phfqit` with the configuration `settings`: for each
/// macro, in a fixed order (the built-in macros of the crate's tables, except
/// each one that a declared macro of the same name replaces, then the declared
/// macros in the order of their names), the macro's element, expanded by the
/// feature's expansion hook, and the lowering rule that has the element
/// expanded in the tree before any output is written; then the settings as an
/// input of the registry digest.
///
/// The lowering rule ([`expand_in_tree_rule_for`]) requires the capability
/// `flm.userdefs.always-expanded` of the standard package's `userdefs` feature,
/// so every plan that lowers one of the macros runs the `userdefs` stage that
/// replaces it by its template, and a registry without that feature fails to
/// build.
///
/// # Errors
///
/// The settings cannot be read, or a declaration under `macro_definitions`
/// cannot be ([`PhfqitData::from_settings`], [`PhfqitConfig::from_data`]),
/// worded as the host reports the refusal of the feature's configuration
/// reader ([`settings::read`]), which reads the configuration the same way
/// before `register` runs; any refusal of the host. A declared macro with the
/// name of a command that another feature defines in the same parsing mode
/// makes the registry build fail with the registry's own error for two
/// declarations of one command.
pub fn register(b: &mut FeatureBuilder, settings: &Value) -> Result<(), ExtError> {
    let data = PhfqitData::from_settings(settings).map_err(settings::refused)?;
    let digest = data.digest_bytes();
    let config = PhfqitConfig::from_data(data).map_err(settings::refused)?;
    let owner = b.feature()?;
    let values = Arc::new(config.values.clone());
    for m in macros(config)? {
        let definition = m.definition.expand(ExpandPolicy::Always);
        let element = element_name_under(PREFIX, &definition)?;
        // The rule covers the element on every backend and schedules the
        // `userdefs` feature's stage that expands the `always` definitions,
        // which makes that feature necessary.
        b.lowering_rule(expand_in_tree_rule_for(&element, &definition)?)?;
        let hook: Arc<dyn Expand> = Arc::new(MacroHook { slots: m.slots, values: Arc::clone(&values) });
        let value = UserDefinition::new(definition)?;
        b.element(value.element_def_with(&owner, &element, hook)?)?;
    }
    b.digest_input(&digest)
}

/// Returns every macro of the feature, in the order they are registered: the
/// built-in macros of the crate's tables, except each built-in macro that a
/// declared macro of the same name replaces, then the declared macros of
/// `config`.
///
/// # Errors
///
/// An argument code of a row is not one, as [`Macro::from_row`] answers it.
fn macros(config: PhfqitConfig) -> Result<Vec<Macro>, ExtError> {
    let declared: BTreeSet<&str> = config.declared.iter().map(|m| m.definition.name.as_str()).collect();
    let mut macros = Vec::new();
    for row in TABLES.iter().flat_map(|table| table.iter()) {
        let m = Macro::from_row(row)?;
        let replaced = m.definition.kind == DefinitionKind::Macro && declared.contains(m.definition.name.as_str());
        if !replaced {
            macros.push(m);
        }
    }
    macros.extend(config.declared);
    Ok(macros)
}

/// Returns the position of `side` in a row of [`MAIN_DELIMITERS`].
fn side_index(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Middle => 1,
        Side::Right => 2,
    }
}

/// The expansion hook ([`Expand`]) of every macro of the feature: it reads the
/// size a use asks for, computes the content of the computed placeholders that
/// the macro's templates write, and instantiates the macro's template.
///
/// A use is an Error at the use, and is replaced by its source text
/// ([`Expander::recover`]), in three cases: the macro is not defined in the
/// use's parsing mode, or its templates did not compile there (the library's
/// `flm.userdefs.bad-definition`, which `UserDefinition::variant_for_use`
/// answers); the size is not a size command or the star, or is given more than
/// once ([`BAD_SIZE`]); a template reads a named value that the settings do not
/// hold ([`UNKNOWN_VALUE`]).
///
/// A computed placeholder whose text does not compile in the use's parsing mode
/// (a delimiter at a size command, such as `\bigl (`, in running text), and a
/// delimiter placeholder written without a delimiter in a macro that has no
/// main delimited argument, have no value: the library reports the placeholder
/// as an Error at the use (`flm.userdefs.bad-placeholder`).
struct MacroHook {
    /// The macro's size slots: each one's argument index and kind.
    slots: Vec<(usize, SlotKind)>,
    /// The named values of the settings, keyed as a template refers to them
    /// (`spaceKets.Bar`), shared by the hooks of every macro of the feature.
    values: Arc<BTreeMap<String, String>>,
}

impl Expand for MacroHook {
    fn policy(&self) -> ExpandPolicy {
        ExpandPolicy::Always
    }

    fn expand(&self, node: &Node, cx: &mut ExpandCx, xp: &mut Expander) -> Result<Expansion, ExtError> {
        // Every element of the feature is declared by `element_def_with`,
        // which registers the definition with the element.
        let Some(definition) = cx.definition()? else { return Ok(Expansion::Default) };
        let variant = match definition.variant_for_use(node) {
            Ok(variant) => variant,
            Err(ExtError::Core { kind, detail, data }) if kind == BAD_DEFINITION => {
                let diagnostic = Diagnostic::error(kind, detail).with_data(data);
                return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
            }
            Err(error) => return Err(error),
        };
        let size = match size_of_use(node, &self.slots)? {
            Ok(size) => size,
            Err(problem) => {
                let name = display_name(&definition.definition()?);
                let diagnostic = Diagnostic::error(BAD_SIZE, problem.message(&name));
                return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
            }
        };
        let template = variant.template()?;
        let mut written = BTreeSet::new();
        let defaults = variant.defaults()?.into_iter().map(|(_, default)| default);
        for compiled in std::iter::once(template).chain(defaults) {
            for reference in compiled.placeholders()? {
                if let PlaceholderTarget::Extra(name) = reference.target {
                    written.insert((name, reference.substitution));
                }
            }
        }
        let mut values = ExtraValues::new();
        if !written.is_empty() {
            let small = SmallTemplates { mode: node.mode()?, lang: xp.lang()? };
            let language = TemplateLanguage::new(&small.lang)?;
            // Read once, at the first delimiter placeholder written without a
            // delimiter.
            let mut main_read = None;
            for (name, substitution) in &written {
                let key = substitution.as_deref().unwrap_or_default();
                let text = if name == CONFIG_PLACEHOLDER {
                    match self.values.get(key) {
                        Some(value) => Some((value.clone(), format!("the value `{key}` of the `phfqit` settings"))),
                        None => {
                            let name = display_name(&definition.definition()?);
                            let diagnostic = Diagnostic::error(UNKNOWN_VALUE, self.unknown_value(key, &name));
                            return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
                        }
                    }
                } else if let Some(side) = SIDES.iter().find(|side| side.placeholder == name) {
                    let delim = if key.is_empty() {
                        let main = match main_read {
                            Some(main) => main,
                            None => *main_read.insert(main_delimiters(node)?),
                        };
                        main.map(|main| main[side_index(side.side)].to_string())
                    } else {
                        Some(key.to_string())
                    };
                    delim.map(|delim| {
                        (sized_delimiter(side.side, size, &delim), format!("the delimiter `{delim}` of a `phfqit` macro"))
                    })
                } else {
                    None
                };
                let Some((text, what)) = text else { continue };
                if let Some(compiled) = small.compile(text, what, &language)? {
                    values = values.template(name, substitution.as_deref(), compiled);
                }
            }
        }
        let bindings = values.over(&DefinitionBindings::new(node, &variant)?)?;
        Ok(Expansion::Emit(xp.instantiate(cx, node, &template, &bindings)?))
    }
}

impl MacroHook {
    /// Returns the message of the Error [`UNKNOWN_VALUE`]: the template of the
    /// macro `name` (as an author writes it) reads the named value `written`,
    /// which the settings do not hold.
    fn unknown_value(&self, written: &str, name: &str) -> String {
        format!(
            "`#[{written}]{{config}}` in the template of `{name}` must refer to one of the values defined in the \
             `phfqit` settings: {}",
            self.values.keys().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
        )
    }
}

/// What the small templates of one use's computed placeholders are compiled
/// against: the use's parsing mode and the built language of the expansion.
struct SmallTemplates {
    /// The parsing mode of the use, the one mode each text is compiled in.
    mode: Mode,
    /// The built language the expansion runs under.
    lang: BuiltLang,
}

impl SmallTemplates {
    /// Returns the template that `text` compiles to in the use's parsing mode,
    /// with the template language `language` of the feature; `what` describes
    /// the text in the template's source. `None` when the text does not compile
    /// in that mode.
    ///
    /// The compilation is the extension interface's: it keeps every template
    /// it compiles for the feature, under the registry in use, and answers the
    /// same template again for the same text, mode and description without
    /// compiling it a second time.
    ///
    /// # Errors
    ///
    /// Any failure of the host other than a text that does not compile, as it
    /// answered.
    // A text is compiled in the use's parsing mode alone, the one mode its
    // instantiation reads it in: a delimiter at a size command parses in
    // formulas only, and makes an Error only at a use in running text.
    fn compile(&self, text: String, what: String, language: &TemplateLanguage) -> Result<Option<CompiledTemplate>, ExtError> {
        let spec = TemplateSpec {
            text,
            mode: self.mode,
            vocabulary: PlaceholderVocabulary::new(0),
            definition: SMALL_TEMPLATES.to_string(),
            label: what.clone(),
            origin_label: what,
            user_definition: false,
            offset: 0,
        };
        match compile_template_with(&spec, &self.lang, language) {
            Ok(template) => Ok(Some(template)),
            Err(ExtError::Core { kind, .. }) if kind == COMPILE_TEMPLATE => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// Returns the three delimiters (left, middle, right) to write around the
/// use's [main delimited argument](MAIN_DELIMITED_ARGUMENT), chosen by the
/// delimiter that opens that argument ([`MAIN_DELIMITERS`]); `None` when the
/// macro declares no such argument or the use does not write it.
///
/// # Errors
///
/// The host's answer to a query of the use's nodes, other than the answer that
/// the macro declares no argument of that name.
fn main_delimiters(node: &Node) -> Result<Option<[&'static str; 3]>, ExtError> {
    let nodes = match node.argument_nodes_named(MAIN_DELIMITED_ARGUMENT) {
        Ok(Some(nodes)) => nodes,
        Ok(None) => return Ok(None),
        Err(ExtError::Core { kind, .. }) if kind == NO_SUCH_ARGUMENT => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut open = None;
    for node in &nodes {
        if let Some((delimiter, _)) = node.group_delimiters()? {
            open = Some(delimiter);
            break;
        }
    }
    let Some(open) = open else { return Ok(None) };
    Ok(MAIN_DELIMITERS.iter().find(|(written, _)| *written == open).map(|(_, delims)| *delims))
}
