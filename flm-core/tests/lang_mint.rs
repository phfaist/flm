//! Minting the parser's packages from element declarations
//! (`flm_core::lang::mint`): clash detection by mode, overrides, scoped
//! packages and their ordering, the fallback package, and the refusals of a
//! layout or a scoped package on an argument or a body whose parser holds no
//! content.

mod common;

use std::sync::Arc;

use flm_core::model::names::FeatureName;
use flm_core::lang::mint::{
    ArgDef, ArgLayout, ArgStdKind, DefinitionShape, FlmArgumentParser, BodyDef, CallableKind, ClashKind, ElementDef, FallbackDef, FallbackKind, MintError,
    OverrideDef, ScopedPackageDef, Symbol, FALLBACK_PACKAGE_NAME, FALLBACK_PROVIDER_NAME,
    FIELDS_PACKAGE_NAME, LANGUAGE_OWNER,
};
use flm_core::lang::{build_language, BuiltLang, Flm, LangBuildError, LangInputs};
use techy::core::constructs::{
    ArgumentParser, ConstructParserResult, MarkerArgumentParser, ParseContext, ParsedArgumentNodes,
};
use techy::core::specs::ArgumentSpec;
use techy::core::ClosedVocabulary;
use techy::latexlike::Mode;

use common::{
    build, callable_names, callables, element, env_def, lang, macro_def, macro_with, parse,
};

fn symbol(kind: CallableKind, name: &str) -> Symbol {
    Symbol { kind, name: name.into() }
}

fn feature_names(names: &[&str]) -> Vec<FeatureName> {
    names.iter().map(|name| FeatureName::from(*name)).collect()
}

/// One clash of the given kind, as `(symbol, modes, owners, first, winner)`.
type Clash = (Symbol, Vec<Mode>, Vec<FeatureName>, FeatureName, FeatureName);

fn clashes(lang: &BuiltLang, wanted: ClashKind) -> Vec<Clash> {
    lang.errors()
        .iter()
        .filter_map(|error| match error {
            LangBuildError::Mint(MintError::Clash {
                kind,
                symbol,
                modes,
                owners,
                first,
                winner,
            }) if *kind == wanted => Some((
                symbol.clone(),
                modes.clone(),
                owners.clone(),
                first.clone(),
                winner.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn built(elements: Vec<ElementDef>) -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.elements = elements;
    build_language(inputs)
}

#[test]
fn a_clash_names_every_owner_with_the_first_and_the_winner() {
    let lang = built(vec![macro_def("alpha", "foo", 0), macro_def("beta", "foo", 0)]);
    let found = clashes(&lang, ClashKind::Symbol);
    assert_eq!(found.len(), 1);
    let (clashed, modes, owners, first, winner) = &found[0];
    assert_eq!(*clashed, symbol(CallableKind::Macro, "foo"));
    // A declaration that lists no mode is available in text mode only.
    assert_eq!(*modes, [Mode::Text]);
    assert_eq!(*owners, feature_names(&["alpha", "beta"]));
    assert_eq!(*first, FeatureName::from("alpha"));
    assert_eq!(*winner, FeatureName::from("beta"));
}

#[test]
fn a_three_feature_clash_names_all_three() {
    let lang = built(vec![
        macro_def("a", "foo", 0),
        macro_def("b", "foo", 0),
        macro_def("c", "foo", 0),
    ]);
    let found = clashes(&lang, ClashKind::Symbol);
    assert_eq!(found.len(), 1);
    let (_, _, owners, first, winner) = &found[0];
    assert_eq!(*owners, feature_names(&["a", "b", "c"]));
    assert_eq!(*first, FeatureName::from("a"));
    assert_eq!(*winner, FeatureName::from("c"));
}

#[test]
fn definitions_in_disjoint_modes_do_not_clash() {
    let lang = built(vec![
        macro_def("a", "vec", 0).modes([Mode::Text]),
        macro_def("b", "vec", 0).modes([Mode::Math]),
    ]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
}

/// The element names of the callable nodes of `text` parsed under `lang`, in
/// document order, and whether the parse reported anything.
fn parsed_elements(lang: &BuiltLang, text: &str) -> (Vec<String>, bool) {
    let fragment = parse(lang, text);
    let elements = callables(&fragment)
        .iter()
        .map(|node| flm_core::model::element::element_of(node).name.to_string())
        .collect();
    (elements, !fragment.diags().is_empty())
}

#[test]
fn a_declaration_without_modes_is_not_found_in_math_mode() {
    let lang = lang(vec![macro_def("a", "foo", 0)]);
    let (elements, _) = parsed_elements(&lang, r"\foo \(\foo\)");
    assert_eq!(elements, ["a.foo", "flm.unknown.macro"]);
}

#[test]
fn a_declaration_in_math_mode_is_not_found_in_text_mode() {
    let lang = lang(vec![macro_def("a", "foo", 0).modes([Mode::Math])]);
    let (elements, _) = parsed_elements(&lang, r"\foo \(\foo\)");
    assert_eq!(elements, ["flm.unknown.macro", "a.foo"]);
}

#[test]
fn a_declaration_in_both_modes_is_found_in_both() {
    let lang = lang(vec![macro_def("a", "foo", 0).modes([Mode::Text, Mode::Math])]);
    let (elements, reported) = parsed_elements(&lang, r"\foo \(\foo\)");
    assert_eq!(elements, ["a.foo", "a.foo"]);
    assert!(!reported);
}

#[test]
fn two_features_declaring_one_name_in_text_and_in_math_do_not_clash() {
    // The first declaration lists no mode: text mode, the default.
    let lang = lang(vec![macro_def("a", "vec", 0), macro_def("b", "vec", 0).modes([Mode::Math])]);
    let (elements, reported) = parsed_elements(&lang, r"\vec \(\vec\)");
    assert_eq!(elements, ["a.vec", "b.vec"]);
    assert!(!reported);
}

#[test]
fn definitions_in_overlapping_modes_clash_in_the_overlapping_modes_only() {
    let lang = built(vec![
        macro_def("a", "vec", 0).modes([Mode::Text]),
        macro_def("b", "vec", 0).modes([Mode::Text, Mode::Math]),
    ]);
    let found = clashes(&lang, ClashKind::Symbol);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, [Mode::Text]);
}

#[test]
fn one_clash_is_reported_per_mode_in_which_two_features_define_a_name() {
    let lang = built(vec![
        macro_def("a", "foo", 0).modes([Mode::Text, Mode::Math]),
        macro_def("b", "foo", 0).modes([Mode::Text, Mode::Math]),
    ]);
    let found = clashes(&lang, ClashKind::Symbol);
    let modes: Vec<Vec<Mode>> = found.iter().map(|clash| clash.1.clone()).collect();
    assert_eq!(modes, [vec![Mode::Text], vec![Mode::Math]]);
    for (_, _, owners, _, winner) in &found {
        assert_eq!(*owners, feature_names(&["a", "b"]));
        assert_eq!(*winner, FeatureName::from("b"));
    }
}

#[test]
fn each_mode_is_checked_with_the_features_that_define_the_name_in_it() {
    // `b` (text only) and `c` (math only) never meet; each meets `a` in one
    // mode, and in each mode the last of the two applies.
    let lang = built(vec![
        macro_def("a", "foo", 0).modes([Mode::Text, Mode::Math]),
        macro_def("b", "foo", 0).modes([Mode::Text]),
        macro_def("c", "foo", 0).modes([Mode::Math]),
    ]);
    let found = clashes(&lang, ClashKind::Symbol);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].1, [Mode::Text]);
    assert_eq!(found[0].2, feature_names(&["a", "b"]));
    assert_eq!(found[0].4, FeatureName::from("b"));
    assert_eq!(found[1].1, [Mode::Math]);
    assert_eq!(found[1].2, feature_names(&["a", "c"]));
    assert_eq!(found[1].4, FeatureName::from("c"));
}

#[test]
fn the_symbols_table_names_the_last_definer_of_a_mode_split_name() {
    let lang = built(vec![
        macro_def("a", "vec", 0).modes([Mode::Text]),
        macro_def("b", "vec", 0).modes([Mode::Math]),
    ]);
    assert_eq!(
        lang.symbols().get(&symbol(CallableKind::Macro, "vec")),
        Some(&FeatureName::from("b"))
    );
}

#[test]
fn one_feature_defining_a_name_twice_in_one_mode_is_reported() {
    // Only one of the two can be in force where they overlap, so the build says
    // so instead of keeping the later declaration alone and silently.
    let lang = built(vec![macro_def("alpha", "foo", 0), macro_def("alpha", "foo", 1)]);
    let reported: Vec<&MintError> = lang
        .errors()
        .iter()
        .filter_map(|error| match error {
            LangBuildError::Mint(error @ MintError::Redeclared { .. }) => Some(error),
            _ => None,
        })
        .collect();
    assert_eq!(reported.len(), 1, "{:?}", lang.errors());
    let MintError::Redeclared { symbol: declared, owner, modes } = reported[0] else {
        unreachable!()
    };
    assert_eq!(declared, &symbol(CallableKind::Macro, "foo"));
    assert_eq!(owner, &FeatureName::from("alpha"));
    assert_eq!(modes, &[Mode::Text]);
}

#[test]
fn one_feature_defining_a_name_in_two_disjoint_modes_is_sound() {
    // Their visibility does not overlap, so both are in force, each in its own
    // mode: the feature's declarations are minted into two of its packages.
    let lang = built(vec![
        macro_def("alpha", "vec", 0).modes([Mode::Text]),
        macro_def("alpha", "vec", 1).modes([Mode::Math]),
    ]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    assert_eq!(
        lang.symbols().get(&symbol(CallableKind::Macro, "vec")),
        Some(&FeatureName::from("alpha"))
    );
}

#[test]
fn an_override_makes_the_redefinition_deliberate() {
    let mut inputs = LangInputs::new();
    inputs.elements = vec![macro_def("alpha", "foo", 0), macro_def("beta", "foo", 0)];
    inputs.overrides =
        vec![OverrideDef { owner: "beta".into(), kind: CallableKind::Macro, name: "foo".into() }];
    let lang = build_language(inputs);
    assert!(lang.is_sound(), "{:?}", lang.errors());
    assert_eq!(
        lang.symbols().get(&symbol(CallableKind::Macro, "foo")),
        Some(&FeatureName::from("beta"))
    );
}

#[test]
fn an_override_by_a_feature_that_is_not_last_is_not_consulted() {
    // Only the override of the definition that applies counts: `alpha`'s is
    // not read, and the collision is reported as any other.
    let mut inputs = LangInputs::new();
    inputs.elements = vec![macro_def("alpha", "foo", 0), macro_def("beta", "foo", 0)];
    inputs.overrides =
        vec![OverrideDef { owner: "alpha".into(), kind: CallableKind::Macro, name: "foo".into() }];
    let lang = build_language(inputs);
    let found = clashes(&lang, ClashKind::Symbol);
    assert_eq!(found.len(), 1, "{:?}", lang.errors());
    assert_eq!(found[0].2, feature_names(&["alpha", "beta"]));
    assert_eq!(found[0].4, FeatureName::from("beta"));
    assert_eq!(lang.errors().len(), 1, "{:?}", lang.errors());
}

#[test]
fn an_override_counts_in_the_modes_where_its_feature_is_last() {
    // `beta` is last in text mode, where its override silences the collision;
    // in math mode `gamma` is last and declares none.
    let mut inputs = LangInputs::new();
    inputs.elements = vec![
        macro_def("alpha", "foo", 0).modes([Mode::Text, Mode::Math]),
        macro_def("beta", "foo", 0).modes([Mode::Text]),
        macro_def("gamma", "foo", 0).modes([Mode::Math]),
    ];
    inputs.overrides =
        vec![OverrideDef { owner: "beta".into(), kind: CallableKind::Macro, name: "foo".into() }];
    let lang = build_language(inputs);
    let found = clashes(&lang, ClashKind::Symbol);
    assert_eq!(found.len(), 1, "{:?}", lang.errors());
    assert_eq!(found[0].1, [Mode::Math]);
    assert_eq!(found[0].2, feature_names(&["alpha", "gamma"]));
}

#[test]
fn a_macro_and_an_environment_of_one_name_do_not_clash() {
    let lang = built(vec![macro_def("alpha", "foo", 0), env_def("beta", "foo", BodyDef::new())]);
    assert!(lang.is_sound(), "{:?}", lang.errors());
}

#[test]
fn an_unknown_scoped_package_is_reported() {
    let lang = built(vec![env_def("t", "list", BodyDef::new().scoped("nope"))]);
    assert!(lang.errors().iter().any(|error| matches!(
        error,
        LangBuildError::Mint(MintError::UnknownScopedPackage { .. })
    )));
}

#[test]
fn a_macro_without_a_name_is_reported() {
    let mut def = macro_def("t", "foo", 0);
    def.names.clear();
    let lang = built(vec![def]);
    assert!(lang
        .errors()
        .iter()
        .any(|error| matches!(error, LangBuildError::Mint(MintError::NoNames { .. }))));
}

// --- a parser of one's own answering that it holds no content ---------------------------

/// An optional `!` marker: a parser of the test's own that holds no document
/// content.
#[derive(Debug)]
struct Bang(MarkerArgumentParser);

impl ArgumentParser<Flm> for Bang {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        self.0.parse_argument(cx, spec)
    }
    fn can_match_empty(&self) -> bool {
        true
    }
}

impl FlmArgumentParser for Bang {
    fn holds_content(&self) -> bool {
        false
    }
    fn identity(&self) -> Vec<u8> {
        b"test.bang".to_vec()
    }
    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::NoShape { token: "bang" }
    }
    fn description(&self) -> String {
        String::from("an optional `!` marker")
    }
}

fn mint_errors(elements: Vec<ElementDef>) -> Vec<MintError> {
    built(elements)
        .errors()
        .iter()
        .filter_map(|error| match error {
            LangBuildError::Mint(error) => Some(error.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_layout_on_a_parser_answering_no_content_is_refused_as_on_a_star() {
    for layout in [ArgLayout::Blocks, ArgLayout::InlineOrBlocks] {
        let bang = ArgDef::named("bang", Bang(MarkerArgumentParser::new("!"))).layout(layout);
        let star = ArgDef::named("star", ArgStdKind::Star).layout(layout);
        let expected = |callable: &str| MintError::LayoutOnNonContentArgument {
            owner: "t".into(),
            callable: callable.into(),
            index: 0,
            field: None,
        };
        assert_eq!(mint_errors(vec![macro_with("t", "b", vec![bang])]), [expected("t.b")]);
        assert_eq!(mint_errors(vec![macro_with("t", "s", vec![star])]), [expected("t.s")]);
    }
}

#[test]
fn a_scoped_package_on_a_parser_answering_no_content_is_refused() {
    let bang = ArgDef::named("bang", Bang(MarkerArgumentParser::new("!"))).scoped("nowhere");
    let errors = mint_errors(vec![macro_with("t", "b", vec![bang])]);
    assert!(
        matches!(errors.as_slice(), [MintError::ScopeOnNonContentArgument { index: 0, .. }]),
        "{errors:?}"
    );
}

#[test]
fn a_parser_answering_no_content_parses_under_its_declaration() {
    let lang = lang(vec![macro_with(
        "t",
        "b",
        vec![ArgDef::named("bang", Bang(MarkerArgumentParser::new("!"))), ArgDef::mandatory("x")],
    )]);
    let fragment = parse(&lang, r"\b!{y}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("!"));
    assert_eq!(node.argument_content_nodes(1).unwrap().source_text(), Some("y"));
    let fragment = parse(&lang, r"\b{y}");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(!node.arguments().unwrap().get(0).unwrap().is_provided());
}

#[test]
fn a_layout_or_a_scoped_package_on_a_verbatim_body_is_refused() {
    let layout = env_def("t", "code", BodyDef::new().verbatim().blocks());
    assert_eq!(
        mint_errors(vec![layout]),
        [MintError::LayoutOnNonContentBody { owner: "t".into(), element: "t.code".into() }]
    );
    let scoped = env_def("t", "code", BodyDef::new().verbatim().scoped("nowhere"));
    assert_eq!(
        mint_errors(vec![scoped]),
        [MintError::ScopeOnNonContentBody { owner: "t".into(), element: "t.code".into() }]
    );
    // The body's layout following the element's role is no declaration.
    assert!(mint_errors(vec![env_def("t", "code", BodyDef::new().verbatim())]).is_empty());
}

// --- scoped packages --------------------------------------------------------------------

fn scoped(owner: &str, name: &str, elements: Vec<ElementDef>) -> ScopedPackageDef {
    ScopedPackageDef { owner: owner.into(), name: name.into(), elements, docs: String::new() }
}

#[test]
fn a_scoped_package_may_name_another_scoped_package() {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages = vec![
        scoped("t", "inner", vec![macro_def("t", "item", 0)]),
        scoped("t", "outer", vec![env_def("t", "sub", BodyDef::new().scoped("inner"))]),
    ];
    inputs.elements = vec![env_def("t", "list", BodyDef::new().scoped("outer"))];
    let lang = build(inputs);
    let fragment = parse(&lang, r"\begin{list}\begin{sub}\item x\end{sub}\end{list}");
    assert_eq!(callable_names(&fragment), ["list", "sub", "item"]);
    assert!(fragment.diags().is_empty());
}

#[test]
fn a_scoped_package_declared_after_the_one_naming_it_is_found() {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages = vec![
        scoped("t", "outer", vec![env_def("t", "sub", BodyDef::new().scoped("inner"))]),
        scoped("t", "inner", vec![macro_def("t", "item", 0)]),
    ];
    inputs.elements = vec![env_def("t", "list", BodyDef::new().scoped("outer"))];
    let lang = build(inputs);
    let fragment = parse(&lang, r"\begin{list}\begin{sub}\item x\end{sub}\end{list}");
    assert_eq!(callable_names(&fragment), ["list", "sub", "item"]);
}

#[test]
fn scoped_packages_naming_each_other_in_a_cycle_are_a_typed_error() {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages = vec![
        scoped("t", "a", vec![env_def("t", "ea", BodyDef::new().scoped("b"))]),
        scoped("t", "b", vec![env_def("t", "eb", BodyDef::new().scoped("a"))]),
    ];
    let lang = build_language(inputs);
    let cycles: Vec<Vec<String>> = lang
        .errors()
        .iter()
        .filter_map(|error| match error {
            LangBuildError::Mint(MintError::ScopedPackageCycle { packages }) => {
                Some(packages.iter().map(|name| name.to_string()).collect())
            }
            _ => None,
        })
        .collect();
    assert_eq!(cycles, [vec!["a".to_string(), "b".to_string()]]);
    assert!(!lang.errors().iter().any(|error| matches!(
        error,
        LangBuildError::Mint(MintError::UnknownScopedPackage { .. })
    )));
}

#[test]
fn a_scoped_package_declared_twice_is_a_clash() {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages = vec![
        scoped("alpha", "items", vec![macro_def("alpha", "item", 0)]),
        scoped("beta", "items", vec![macro_def("beta", "item", 0)]),
    ];
    let lang = build_language(inputs);
    let found = clashes(&lang, ClashKind::ScopedPackage);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].2, feature_names(&["alpha", "beta"]));
}

/// Two features each declare `\\` in text mode in a scoped package of its own:
/// the scoped packages are never checked against one another, so the language
/// builds; each `\\` resolves to its own feature's element inside that feature's
/// environment, and the inner package shadows the outer one where one environment
/// stands inside the other.
#[test]
fn two_scoped_packages_declaring_one_name_do_not_clash() {
    let mut inputs = LangInputs::new();
    inputs.scoped_packages = vec![
        scoped("alpha", "alpha.lines", vec![macro_def("alpha", "\\", 0).modes([Mode::Text])]),
        scoped("beta", "beta.lines", vec![macro_def("beta", "\\", 0).modes([Mode::Text])]),
    ];
    inputs.elements = vec![
        env_def("alpha", "rows", BodyDef::new().scoped("alpha.lines")),
        env_def("beta", "lines", BodyDef::new().scoped("beta.lines")),
    ];
    let lang = build(inputs);
    let fragment = parse(
        &lang,
        r"\begin{rows}a\\b\begin{lines}c\\d\end{lines}e\\f\end{rows}\begin{lines}g\\h\end{lines}",
    );
    let elements: Vec<String> = callables(&fragment)
        .iter()
        .map(|node| flm_core::model::element::element_of(node).name.to_string())
        .collect();
    assert_eq!(
        elements,
        [
            "alpha.rows",
            "alpha.\\",
            "beta.lines",
            "beta.\\",
            "alpha.\\",
            "beta.lines",
            "beta.\\",
        ]
    );
    assert!(fragment.diags().is_empty());
}

// --- fallbacks ---------------------------------------------------------------------------

fn fallback(owner: &str, modes: Vec<Mode>, kind: FallbackKind) -> FallbackDef {
    FallbackDef { owner: owner.into(), modes, kind, element: element(&format!("{owner}.any")) }
}

#[test]
fn two_fallbacks_for_one_mode_and_kind_clash_naming_the_modes() {
    let mut inputs = LangInputs::new();
    inputs.fallbacks = vec![
        fallback("alpha", vec![Mode::Math], FallbackKind::Macro),
        fallback("beta", vec![Mode::Math], FallbackKind::Macro),
    ];
    let lang = build_language(inputs);
    let found = clashes(&lang, ClashKind::Fallback);
    assert_eq!(found.len(), 1);
    let (_, modes, owners, first, winner) = &found[0];
    assert_eq!(modes, &[Mode::Math]);
    assert_eq!(*owners, feature_names(&["alpha", "beta"]));
    assert_eq!(*first, FeatureName::from("alpha"));
    assert_eq!(*winner, FeatureName::from("alpha"));
}

#[test]
fn fallbacks_for_disjoint_modes_do_not_clash() {
    let mut inputs = LangInputs::new();
    inputs.fallbacks = vec![
        fallback("alpha", vec![Mode::Math], FallbackKind::Macro),
        fallback("beta", vec![Mode::Text], FallbackKind::Macro),
    ];
    let lang = build(inputs);
    assert!(lang.fallback().answers(FallbackKind::Macro, Mode::Math));
    assert!(lang.fallback().answers(FallbackKind::Macro, Mode::Text));
}

#[test]
fn fallback_specs_carry_the_fallback_package_provenance() {
    let mut inputs = LangInputs::new();
    inputs.fallbacks = vec![fallback("math", vec![Mode::Math], FallbackKind::Macro)];
    let lang = build(inputs);
    let fragment = parse(&lang, r"\(\alpha\)");
    let node = callables(&fragment).into_iter().next().unwrap();
    let provenance = node.spec().unwrap().provenance().unwrap();
    let provider = provenance.provider().unwrap();
    assert_eq!(provider.name(), FALLBACK_PACKAGE_NAME);
    assert!(Arc::ptr_eq(lang.fallback().package(), &lang.fallback().package().clone()));
}

#[test]
fn the_language_layers_environment_fallback_is_installed_last_for_every_mode() {
    let lang = lang(Vec::new());
    let (owner, modes, kind) = lang.fallback().entries().last().unwrap();
    assert_eq!(*owner, FeatureName::from(LANGUAGE_OWNER));
    assert_eq!(modes, Mode::ALL);
    assert_eq!(kind, FallbackKind::Environment);
}

#[test]
fn a_feature_environment_fallback_shadows_the_language_layers_in_its_modes() {
    let mut inputs = LangInputs::new();
    inputs.fallbacks = vec![fallback("math", vec![Mode::Math], FallbackKind::Environment)];
    let lang = build(inputs);
    let fragment = parse(&lang, r"\(\begin{nope}x\end{nope}\)");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(flm_core::model::element::element_of(&node).name.as_str(), "math.any");
    assert!(fragment.diags().is_empty());
}

#[test]
fn the_providers_list_the_fallback_gate_and_package_then_every_minted_package() {
    let lang = built(vec![macro_def("alpha", "foo", 0), macro_def("beta", "bar", 0)]);
    let names: Vec<&str> = lang.providers().iter().map(|provider| provider.name()).collect();
    assert_eq!(
        names,
        [FALLBACK_PROVIDER_NAME, FALLBACK_PACKAGE_NAME, FIELDS_PACKAGE_NAME, "alpha", "beta"]
    );
}
