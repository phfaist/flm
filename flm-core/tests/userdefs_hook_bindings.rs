//! A feature's own expansion hook over a definition: extra placeholder names
//! declared on the `Definition` (`Definition::extra_placeholder`), compiled with
//! the definition's templates, and answered by bindings of the feature's own
//! that hand every other reference to the definition's base bindings
//! (`DefinitionBindings`) — written against the public interface alone.

mod common;

use std::sync::Arc;

use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::userdefs::{
    compile_template, expand, BadPlaceholder, Definition, DefinitionBindings, Expand, ExpandCx,
    ExpandError, ExpandOptions, ExpandSet, Expander, Expansion, PlaceholderBindings,
    PlaceholderTarget, PlaceholderValue, PlaceholderVocabulary, TemplateSpec, UserDefinition,
};
use techy::latexlike::Mode;
use flm_core::lang::{build_language, BuiltLang, Flm, FlmDiagnostics, LangInputs};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use techy::core::node::{NodeRef, NodeSlice, NodeTree};

use common::parse;
use common::userdefs::recomposed;

/// A hook answering `#[…]{twice}` with its substitution argument (as the value)
/// and `#{stamp}` with a tree of its own, over a user definition's compiled
/// templates.
struct Twice {
    definition: Arc<UserDefinition>,
    stamp: Arc<NodeTree<Flm>>,
}

struct TwiceBindings<'t> {
    base: DefinitionBindings<'t>,
    stamp: &'t NodeTree<Flm>,
}

impl PlaceholderBindings for TwiceBindings<'_> {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        match target {
            PlaceholderTarget::Extra(name) if name == "twice" => PlaceholderValue::Substitution,
            PlaceholderTarget::Extra(name) if name == "stamp" => PlaceholderValue::Tree(self.stamp),
            _ => self.base.value(target, substitution),
        }
    }
}

impl Expand for Twice {
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        let mode = node.parsing_state().mode();
        let compiled = self.definition.compiled().ok_or_else(|| ExpandError::Hook {
            detail: String::from("not compiled"),
        })?;
        let variant = compiled.variant(mode).ok_or_else(|| ExpandError::Hook {
            detail: String::from("no template for this mode"),
        })?;
        let bindings =
            TwiceBindings { base: DefinitionBindings::new(node, variant), stamp: &self.stamp };
        Ok(Expansion::Emit(xp.instantiate(cx, node, variant.template(), &bindings)?))
    }
}

/// `\tw{a}[o]`: a mandatory and an optional argument, the latter with a
/// default, and the template `template`.
fn definition(template: &str) -> Definition {
    Definition::macro_("tw", vec![ArgDef::mandatory("a"), ArgDef::optional("o")])
        .content(template)
        .default("o", "D")
        .extra_placeholder("twice", true)
        .extra_placeholder("stamp", false)
}

fn language(template: &str) -> (BuiltLang, Arc<UserDefinition>) {
    let value = UserDefinition::new(definition(template));
    let parsed = {
        let mut inputs = LangInputs::new();
        inputs.elements.push(value.element_def("x", "x.tw"));
        build_language(inputs)
    };
    let stamp = Arc::new(parse(&parsed, "STAMP").tree().annotate(|_| ()));
    let hook: Arc<dyn Expand> = Arc::new(Twice { definition: value.clone(), stamp });
    let spec = Arc::new(ElementSpec::new("x.tw").with::<dyn Expand>(hook));
    let mut inputs = LangInputs::new();
    let def = value.element_def("x", "x.tw");
    inputs.elements.push(ElementDef::new("x", def.names, def.syntax, spec));
    let lang = build_language(inputs);
    assert!(lang.is_sound());
    let warnings = value.compile(&lang).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(warnings.is_empty(), "{warnings:?}");
    (lang, value)
}

fn expand_text(lang: &BuiltLang, text: &str) -> (String, FlmDiagnostics) {
    let fragment = parse(lang, text);
    let mut diagnostics = FlmDiagnostics::new();
    let tree = expand(
        fragment.tree(),
        lang,
        &ExpandOptions { recursion_limit: 8, guard: lang.guard() },
        ExpandSet::Both,
        &mut diagnostics,
    )
    .expect("an expanded tree");
    (recomposed(&tree), diagnostics)
}

#[test]
fn a_hook_answers_its_own_names_and_the_base_bindings_the_arguments_and_defaults() {
    let (lang, _) = language("<#1|#2|#[x]{twice}#[y]{twice}|#{stamp}>");
    let (text, diagnostics) = expand_text(&lang, r"\tw{A}[B] \tw{C}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "<A|B|xy|STAMP> <C|D|xy|STAMP>");
}

#[test]
fn a_name_not_declared_on_the_definition_refuses_its_template() {
    let value = UserDefinition::new(
        Definition::macro_("tw", Vec::new()).content("#{nothing}").extra_placeholder("stamp", false),
    );
    let mut inputs = LangInputs::new();
    inputs.elements.push(value.element_def("x", "x.tw"));
    let lang = build_language(inputs);
    let errors = value.compile(&lang).expect_err("an unknown placeholder name");
    assert!(errors.iter().any(|diagnostic| diagnostic.identifier() == "flm.userdefs.bad-placeholder"));
}

#[test]
fn the_definitions_own_hook_leaves_an_extra_name_unresolvable() {
    // Registered as a plain user definition, whose own hook answers no extra
    // name: the use site recovers with `flm.userdefs.bad-placeholder`.
    let value = UserDefinition::new(definition("[#{stamp}]"));
    let mut inputs = LangInputs::new();
    inputs.elements.push(value.element_def("x", "x.tw"));
    let lang = build_language(inputs);
    value.compile(&lang).unwrap_or_else(|errors| panic!("{errors:?}"));
    let (text, diagnostics) = expand_text(&lang, r"\tw{A}");
    assert_eq!(text, r"\tw{A}");
    let conditions: Vec<&BadPlaceholder> = diagnostics.conditions::<BadPlaceholder>().collect();
    assert_eq!(conditions.len(), 1);
}

#[test]
fn the_extra_names_are_part_of_the_configurations_digest() {
    let plain = UserdefsConfig::new()
        .definition(Definition::macro_("tw", Vec::new()).content("x"))
        .digest_bytes();
    let with_extra = UserdefsConfig::new()
        .definition(Definition::macro_("tw", Vec::new()).content("x").extra_placeholder("s", false))
        .digest_bytes();
    let with_substitution = UserdefsConfig::new()
        .definition(Definition::macro_("tw", Vec::new()).content("x").extra_placeholder("s", true))
        .digest_bytes();
    assert_ne!(plain, with_extra);
    assert_ne!(with_extra, with_substitution);
}

/// A hook that computes its template at the use site, from the use site's
/// parsing mode, and compiles it against the built language the expander gives.
struct Computed;

struct NoBindings;

impl PlaceholderBindings for NoBindings {
    fn value<'a>(
        &'a self,
        _target: &PlaceholderTarget,
        _substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        PlaceholderValue::NoValue
    }
}

impl Expand for Computed {
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        let vocabulary = PlaceholderVocabulary::new(0);
        let text = format!("<{}>", node.parsing_state().mode() == Mode::Text);
        let spec = TemplateSpec {
            text: &text,
            mode: Mode::Text,
            vocabulary: &vocabulary,
            offset: 0,
            definition: "\\cp",
            label: "expansion of \\cp",
            origin_label: "template of \\cp",
            user_definition: false,
        };
        let template = compile_template(&spec, xp.lang())
            .map_err(|errors| ExpandError::Hook { detail: format!("{errors:?}") })?;
        Ok(Expansion::Emit(xp.instantiate(cx, node, &template, &NoBindings)?))
    }
}

#[test]
fn an_expansion_hook_reads_the_built_language_through_the_expander() {
    let hook: Arc<dyn Expand> = Arc::new(Computed);
    let spec = Arc::new(ElementSpec::new("x.cp").with::<dyn Expand>(hook));
    let mut inputs = LangInputs::new();
    inputs.elements.push(ElementDef::new("x", ["cp"], SyntaxDef::Macro { args: Vec::new() }, spec));
    let lang = build_language(inputs);
    assert!(lang.is_sound());
    let (text, diagnostics) = expand_text(&lang, r"a \cp{} b \cp{}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "a <true>{} b <true>{}");
}
