//! Settings given as data: each standard feature's declaration of its
//! settings, the reading of every key, the problems a reading finds, the
//! schema, the package factory of `stdcore` and the package catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use flm_core::core::registry::{
    PackageCatalog, PackageEntry, PackageFactory, PackageResolver, RegistryBuilder,
    RegistryInputs, ResolveError,
};
use flm_core::core::settings::{
    FeatureSettings, PackageSettings, SettingsDecl, SettingsErrorKind,
};
use flm_core::lang::mint::{arg_def_from_code, ArgDef, ArgLayout};
use flm_core::lang::rewrites::blocks::BlocksConfig;
use flm_core::model::names::Name;
use flm_core::model::options::{Bind, OptionKey, Options};
use flm_core::stdcore::backends::html::{DefinitionOverride, HtmlRenderOptions};
use flm_core::stdcore::backends::latex::{ExtraPreamble, LatexPackageDef, UnicodeMode};
use flm_core::stdcore::backends::latex_recomposed::{LatexRecomposedRenderOptions, StyleFile};
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
use flm_core::stdcore::backends::text::TextRenderOptions;
use flm_core::stdcore::features::annotations::{
    AnnotationsConfig, AnnotationsRenderOptions, Annotator,
};
use flm_core::stdcore::features::cells::CellsConfig;
use flm_core::stdcore::features::citations::{
    CitationPlacement, CitationsConfig, MarkForm, STYLE_NAME, default_style,
};
use flm_core::stdcore::features::defterm::DeftermConfig;
use flm_core::stdcore::features::floats::{FloatType, FloatsConfig};
use flm_core::stdcore::features::graphics::GraphicsConfig;
use flm_core::stdcore::features::headings::latex::HeadingsLatexOptions;
use flm_core::stdcore::features::headings::{
    HeadingCommand, HeadingDecl, HeadingNumbering, HeadingsConfig,
};
use flm_core::stdcore::features::lists::{ListKind, ListsConfig};
use flm_core::stdcore::features::math::{MathConfig, MathRecomposeOptions};
use flm_core::stdcore::features::numbering::{
    CounterOptions, CounterStyleDecl, Delimiters, JoinDecl, NumberTemplate, NumberingConfig,
    NumberingOptions, PrefixWords, WithinJoinerSpec,
};
use flm_core::stdcore::features::quote::{QuoteConfig, QuoteEnvironment, QuoteSection};
use flm_core::stdcore::features::refs::{
    RefsConfig, RefsRecomposeOptions, RefsRenderOptions, SafeLabelMode, UnresolvedRefs,
};
use flm_core::stdcore::features::theorems::{TheoremClass, TheoremKind, TheoremsConfig};
use flm_core::model::aspects::BlockRole;
use flm_core::stdcore::features::userdefs::{Definition, ExpandPolicy, UserdefsConfig};
use flm_core::stdcore::features::xnotes::{XnoteKind, XnotesConfig};
use flm_core::untechxt::preamble::EngineSet;
use flm_core::stdcore::{self, StdcoreConfig};
use serde_json::{Value, json};

/// The factory of the standard package.
fn factory() -> PackageFactory {
    stdcore::factory().expect("the standard package builds from empty settings")
}

/// The declaration of the standard feature `feature`.
fn decl(feature: &str) -> SettingsDecl {
    factory().settings(feature).cloned().unwrap_or_else(|| panic!("`{feature}` takes settings"))
}

/// The settings of `feature` read from `data`, which must read, and which the
/// schema of the feature's settings must accept (test 3).
fn read(feature: &str, data: Value) -> FeatureSettings {
    assert!(schema_accepts(feature, &data), "the schema of `{feature}` refuses {data}");
    let decl = decl(feature);
    match decl.read(data.as_object().expect("a map")) {
        Ok(settings) => settings,
        Err(errors) => panic!("`{feature}` did not read: {errors:?}"),
    }
}

/// The configuration of type `C` read for `feature` from `data`.
fn config<C: Clone + 'static>(feature: &str, data: Value) -> C {
    read(feature, data).config::<C>().cloned().expect("a configuration of this type")
}

/// The option value of type `O` read for `feature` from `data`.
fn option<O: Options + Clone>(feature: &str, data: Value) -> O {
    let settings = read(feature, data);
    settings
        .render_options()
        .get::<O>()
        .or_else(|| settings.document_options().get::<O>())
        .cloned()
        .expect("an option value of this type")
}

/// Two values are the same when their debug forms are: some configurations
/// hold values that do not compare.
fn same<T: std::fmt::Debug>(read: &T, built: &T) {
    assert_eq!(format!("{read:?}"), format!("{built:?}"));
}

// ---- test 1: every key reads as the Rust interface builds -----------------

#[test]
fn blocks_reads_every_key() {
    let read = config::<BlocksConfig>(
        "blocks",
        json!({
            "auto_unicode_quotes": false,
            "ligature_unicode_quotes": false,
            "ligature_unicode_dashes": false,
            "ligature_unicode_ellipses": false,
        }),
    );
    assert_eq!(read, BlocksConfig::no_typography());
}

#[test]
fn math_reads_every_key() {
    assert_eq!(
        config::<MathConfig>("math", json!({ "numbered_equations": false })),
        MathConfig { numbered_equations: false }
    );
    let read = option::<MathRecomposeOptions>(
        "math",
        json!({
            "surround_display_math": ["\\[", "\\]"],
            "emit_flm_math_environment_macro": true,
        }),
    );
    let built = MathRecomposeOptions {
        surround_display_math: ("\\[".into(), "\\]".into()),
        emit_flm_math_environment_macro: true,
    };
    assert_eq!(read, built);
}

#[test]
fn graphics_cells_and_defterm_read_every_key() {
    assert_eq!(config::<GraphicsConfig>("graphics", json!({ "global": true })), GraphicsConfig { global: true });
    assert_eq!(config::<CellsConfig>("cells", json!({ "global": true })), CellsConfig { global: true });
    assert_eq!(config::<DeftermConfig>("defterm", json!({ "lead_in": null })), DeftermConfig { lead_in: None });
}

#[test]
fn refs_reads_every_key() {
    let read = config::<RefsConfig>("refs", json!({ "join": { "and": "und", "sep": "; " } }));
    let built = RefsConfig {
        join: JoinDecl { and: Some("und".into()), sep: Some("; ".into()), ..JoinDecl::default() },
    };
    same(&read, &built);
    assert_eq!(
        option::<RefsRenderOptions>("refs", json!({ "unresolved": "allow" })),
        RefsRenderOptions { unresolved: UnresolvedRefs::Allow }
    );
    let read = option::<RefsRecomposeOptions>(
        "refs",
        json!({ "safe_label_types": { "eq": "raw" }, "protect": true }),
    );
    let built = RefsRecomposeOptions {
        safe_label_types: BTreeMap::from([("eq".to_string(), SafeLabelMode::Raw)]),
        protect: true,
    };
    assert_eq!(read, built);
}

#[test]
fn citations_reads_every_key() {
    let read = config::<CitationsConfig>(
        "citations",
        json!({
            "placement": "in-place",
            "mark": "superscript",
            "style": {
                "number_format": { "formats": "#{roman}" },
                "sub_levels": [],
                "prefix_words": { "base": { "one": "cit.~", "other": "cits.~" } },
                "delimiters": { "open": "(", "close": ")" },
                "join": {},
                "name_in_link": false,
                "repeat_parents_in_range": false,
                "ranges": false,
                "number_writer": null,
            },
            "heading_title": null,
            "heading_level": 3,
            "note_separator": ", ",
        }),
    );
    let mut style = default_style();
    style.number_format = NumberTemplate::formats("#{roman}");
    style.prefix_words = PrefixWords::new("cit.~", "cits.~");
    style.delimiters.open = "(".into();
    style.delimiters.close = ")".into();
    style.join = JoinDecl::default();
    style.name_in_link = false;
    style.ranges = false;
    let built = CitationsConfig {
        placement: CitationPlacement::InPlace,
        mark: MarkForm::Superscript,
        style,
        heading_title: None,
        heading_level: 3,
        note_separator: ", ".into(),
    };
    assert_eq!(read, built);
}

#[test]
fn the_citation_style_is_read_over_the_default_style() {
    let read = config::<CitationsConfig>(
        "citations",
        json!({ "style": { "delimiters": { "open": "(", "close": ")" } } }),
    );
    let mut built = CitationsConfig::new();
    built.style.delimiters = Delimiters::new("(", ")");
    assert_eq!(read, built);
    assert_eq!(read.style.name, STYLE_NAME);

    // A style's name is never written: it is the feature's, or the key.
    let errors = decl("citations")
        .read(json!({ "style": { "name": "my.style" } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "citations.style");
    let errors = decl("numbering")
        .read(json!({ "styles": { "my.style": { "name": "my.style" } } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "numbering.styles.my.style");
}

#[test]
fn annotations_reads_every_key() {
    let read = config::<AnnotationsConfig>(
        "annotations",
        json!({ "annotators": { "pf": { "initials": "PF" }, "vva": {} } }),
    );
    let mut first = Annotator::new("pf");
    first.initials = Some("PF".into());
    let built = AnnotationsConfig { annotators: vec![first, Annotator::new("vva")] };
    assert_eq!(read, built);
    assert_eq!(
        option::<AnnotationsRenderOptions>("annotations", json!({ "hidden": true })),
        AnnotationsRenderOptions { hidden: true }
    );
}

#[test]
fn numbering_reads_every_key() {
    let read = config::<NumberingConfig>(
        "numbering",
        json!({ "styles": { "my.style.b": { "ranges": false }, "my.style.a": {} } }),
    );
    let mut first = CounterStyleDecl::new("my.style.b");
    first.ranges = false;
    let built = NumberingConfig { styles: vec![first, CounterStyleDecl::new("my.style.a")] };
    assert_eq!(read, built, "the entries stand in the order written");
    let read = option::<NumberingOptions>(
        "numbering",
        json!({ "counters": { "flm.counter.section": { "initial": 3 } } }),
    );
    let built = NumberingOptions {
        counters: BTreeMap::from([(
            Name::from("flm.counter.section"),
            CounterOptions { within: None, initial: Some(3) },
        )]),
    };
    assert_eq!(read, built);
    let settings = read_settings_of_either();
    assert!(settings.document_options().get::<NumberingOptions>().is_some());
    assert!(settings.render_options().get::<NumberingOptions>().is_none());
}

/// The numbering options, bound either per document or per render, are set on
/// the document.
fn read_settings_of_either() -> FeatureSettings {
    read("numbering", json!({ "counters": {} }))
}

#[test]
fn headings_reads_every_key() {
    let read = config::<HeadingsConfig>(
        "headings",
        json!({
            "headings": { "chapter": { "level": 1, "run_in": true } },
            "commands": { "chapter": { "heading": "chapter" } },
            "numbering": { "up-to-depth": 2 },
        }),
    );
    let mut chapter = HeadingDecl::new("chapter", 1);
    chapter.run_in = true;
    let built = HeadingsConfig {
        headings: vec![chapter],
        commands: vec![HeadingCommand::new("chapter", "chapter")],
        numbering: HeadingNumbering::UpToDepth(2),
    };
    assert_eq!(read, built);
    assert_eq!(
        option::<HeadingsLatexOptions>("headings", json!({ "latex_commands": { "section": "chapter" } })),
        HeadingsLatexOptions::new().command("section", "chapter")
    );
}

#[test]
fn lists_reads_every_key() {
    let read = config::<ListsConfig>(
        "lists",
        json!({
            "environments": {
                "steps": {
                    "numbered": true,
                    "labels": ["#{arabic}."],
                    "ref_labels": ["#{arabic}"],
                    "nesting": { "joined": "." },
                },
            },
        }),
    );
    let built = ListsConfig {
        environments: BTreeMap::from([(
            Name::from("steps"),
            ListKind {
                numbered: true,
                labels: vec!["#{arabic}.".into()],
                ref_labels: vec!["#{arabic}".into()],
                nesting: WithinJoinerSpec::Joined(".".into()),
            },
        )]),
    };
    assert_eq!(read, built);
}

#[test]
fn xnotes_reads_every_key() {
    let read = config::<XnotesConfig>(
        "xnotes",
        json!({
            "kinds": {
                "endnote": {
                    "words": { "base": { "one": "endnote", "other": "endnotes" } },
                    "commands": ["endnote"],
                    "placement": "in-place",
                },
            },
        }),
    );
    let mut endnote = XnoteKind::numbered("endnote", PrefixWords::new("endnote", "endnotes"));
    endnote.commands = vec!["endnote".into()];
    endnote.placement = flm_core::stdcore::features::xnotes::XnotePlacement::InPlace;
    assert_eq!(read, XnotesConfig { kinds: vec![endnote] });
}

#[test]
fn theorems_reads_every_key() {
    let read = config::<TheoremsConfig>(
        "theorems",
        json!({
            "classes": {
                "plain": {
                    "numbered": true,
                    "shared_counter": false,
                    "heading": "bold",
                    "closing_mark": null,
                    "relation_ref": false,
                },
            },
            "kinds": {
                "claim": {
                    "class": "plain",
                    "words": { "base": { "one": "Claim~", "other": "Claims~" } },
                    "expects_proof": true,
                },
            },
            "number_format": { "initial": "a" },
            "label_types": ["claim"],
        }),
    );
    let mut claim = TheoremKind::new("plain", PrefixWords::new("Claim~", "Claims~"));
    claim.expects_proof = true;
    let built = TheoremsConfig {
        classes: BTreeMap::from([(
            Name::from("plain"),
            TheoremClass {
                numbered: true,
                shared_counter: false,
                heading: "bold".into(),
                closing_mark: None,
                relation_ref: false,
            },
        )]),
        kinds: BTreeMap::from([(Name::from("claim"), claim)]),
        number_format: NumberTemplate::initial("a"),
        label_types: vec!["claim".into()],
    };
    assert_eq!(read, built);
}

#[test]
fn quote_reads_every_key() {
    let read = config::<QuoteConfig>(
        "quote",
        json!({ "environments": { "verse": { "sections": ["lines"], "bare": "lines" } } }),
    );
    let mut verse = QuoteEnvironment::new([QuoteSection::Lines]);
    verse.bare = Some(QuoteSection::Lines);
    assert_eq!(read, QuoteConfig { environments: BTreeMap::from([("verse".to_string(), verse)]) });
}

#[test]
fn floats_reads_every_key() {
    let read = config::<FloatsConfig>(
        "floats",
        json!({ "types": { "diagram": { "handlers": ["includegraphics"], "numbering": "unnumbered" } } }),
    );
    let mut diagram = FloatType::new("diagram", ["includegraphics"]);
    diagram.numbering = flm_core::stdcore::features::floats::FloatNumbering::Unnumbered;
    assert_eq!(read, FloatsConfig { types: vec![diagram] });
}

#[test]
fn a_value_with_a_text_name_is_written_as_that_name() {
    for style_file in [StyleFile::WhenNeeded, StyleFile::Always] {
        let read = option::<LatexRecomposedRenderOptions>(
            "latex-recomposed",
            json!({ "style_file": style_file.as_str() }),
        );
        assert_eq!(read.style_file, style_file);
    }
    use flm_core::stdcore::features::floats::FloatNumbering;
    for numbering in [FloatNumbering::Numbered, FloatNumbering::Unnumbered, FloatNumbering::AutoNumbered] {
        let data = json!({ "types": { "chart": { "handlers": [], "numbering": numbering.as_str() } } });
        assert_eq!(config::<FloatsConfig>("floats", data).types[0].numbering, numbering);
    }
    // The expansion policy's names are those the registry digest and the
    // guides write.
    for (name, expand) in [("always", ExpandPolicy::Always), ("per-plan", ExpandPolicy::PerPlan)] {
        let data = json!({ "macros": { "hi": { "content": "", "expand": name } } });
        assert_eq!(config::<UserdefsConfig>("userdefs", data).definitions[0].expand, expand);
    }
}

#[test]
fn an_entry_requires_in_data_what_its_constructor_requires() {
    // What the constructor takes is written; every other field keeps the
    // value the constructor gives for the entry's name.
    let read = config::<HeadingsConfig>("headings", json!({ "headings": { "part": { "level": 0 } } }));
    assert_eq!(read.headings, vec![HeadingDecl::new("part", 0)]);
    let read = config::<FloatsConfig>("floats", json!({ "types": { "chart": { "handlers": ["cells"] } } }));
    assert_eq!(read.types, vec![FloatType::new("chart", ["cells"])]);
    let words = json!({ "base": { "one": "aside", "other": "asides" } });
    let read = config::<XnotesConfig>("xnotes", json!({ "kinds": { "aside": { "words": words } } }));
    assert_eq!(read.kinds, vec![XnoteKind::numbered("aside", PrefixWords::new("aside", "asides"))]);

    // An entry that does not write it is not read.
    for (feature, data, path) in [
        ("headings", json!({ "headings": { "part": { "run_in": true } } }), "headings.headings.part"),
        ("floats", json!({ "types": { "chart": {} } }), "floats.types.chart"),
        ("xnotes", json!({ "kinds": { "aside": { "commands": ["aside"] } } }), "xnotes.kinds.aside"),
    ] {
        let errors = decl(feature).read(data.as_object().unwrap()).unwrap_err();
        assert_eq!(errors.len(), 1, "{feature}");
        assert_eq!(errors[0].path, path);
        assert_eq!(errors[0].kind, SettingsErrorKind::InvalidValue);
    }
}

/// The argument declaration read from the argument code `code`, which keeps
/// the code as the settings' reader does.
fn coded(code: &str) -> ArgDef {
    arg_def_from_code(code).unwrap()
}

/// The argument declaration read from the argument code `code`, named `name`.
fn named(name: &str, code: &str) -> ArgDef {
    let mut arg = coded(code);
    arg.name = Some(name.to_string());
    arg
}

#[test]
fn userdefs_reads_every_key_and_the_argument_codes() {
    let read = config::<UserdefsConfig>(
        "userdefs",
        json!({
            "macros": {
                "hi": {
                    "content": "Hello #1 #2 #3",
                    "args": ["{", "[", { "code": "*", "name": "star" }],
                    "defaults": { "2": "there" },
                    "argument_number_offset": 0,
                    "block_role": "block",
                    "expand": "always",
                    "overrides": true,
                    "docs": "Greets.",
                },
                "vec": { "content": { "text": "arrow", "math": "\\vec{#1}" }, "args": ["{"] },
            },
            "environments": {
                "hi": { "content": "#{body}", "args": [{ "code": "m", "layout": "blocks" }] },
            },
            "specials": { "~~": { "content": { "math": "\\sim" } } },
            "recursion_limit": 4,
        }),
    );
    let built = UserdefsConfig::new()
        .definition(
            Definition::macro_(
                "hi",
                vec![coded("{"), coded("["), named("star", "*")],
            )
            .content("Hello #1 #2 #3")
            .default("2", "there")
            .argument_number_offset(0)
            .block_role(BlockRole::Block)
            .expand(ExpandPolicy::Always)
            .overriding()
            .docs("Greets."),
        )
        .definition(Definition::macro_("vec", vec![coded("{")]).text("arrow"))
        .definition(Definition::macro_("vec", vec![coded("{")]).math("\\vec{#1}"))
        .definition(
            Definition::environment("hi", vec![coded("m").layout(ArgLayout::Blocks)])
                .content("#{body}"),
        )
        .definition(Definition::specials("~~", Vec::new()).math("\\sim"))
        .recursion_limit(4);
    same(&read, &built);
}

#[test]
fn a_user_definition_by_mode_must_give_a_mode() {
    let errors = decl("userdefs")
        .read(json!({ "macros": { "hi": { "content": {} } } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "userdefs.macros.hi");
    let errors = decl("userdefs")
        .read(json!({ "definitions": {} }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].kind, SettingsErrorKind::UnknownKey);
}

#[test]
fn the_backends_read_every_key() {
    let read = option::<PiecesRenderOptions>(
        "pieces",
        json!({ "include": ["flm.xnotes.footnote"], "exclude": ["flm.citations.list"] }),
    );
    let built = PiecesRenderOptions {
        include: BTreeSet::from([Name::from("flm.xnotes.footnote")]),
        exclude: BTreeSet::from([Name::from("flm.citations.list")]),
        // Not a setting: a host sets it from the template it places the
        // output with.
        backends: Default::default(),
    };
    assert_eq!(read, built);

    let read = option::<TextRenderOptions>(
        "text",
        json!({
            "width": 60, "tab_width": 4, "run_in_stop": ":", "link_addresses": false,
            "link_addresses_when_shown": true, "root_heading_level": 2,
        }),
    );
    let built = TextRenderOptions {
        width: Some(60),
        tab_width: 4,
        run_in_stop: ":".into(),
        link_addresses: false,
        link_addresses_when_shown: true,
        root_heading_level: 2,
    };
    assert_eq!(read, built);

    let read = option::<HtmlRenderOptions>(
        "html",
        json!({
            "title": "A page", "lang": "en", "source_spans": true, "run_in_stop": ":",
            "definitions": { "flm.html.math": "provided-by-host" },
            "required_definitions": ["flm.html.mathjax"],
            "heading_tags": { "section": "h2" }, "root_heading_level": 2,
        }),
    );
    let built = HtmlRenderOptions {
        title: "A page".into(),
        lang: Some("en".into()),
        source_spans: true,
        run_in_stop: ":".into(),
        definitions: BTreeMap::from([(Name::from("flm.html.math"), DefinitionOverride::ProvidedByHost)]),
        required_definitions: BTreeSet::from([Name::from("flm.html.mathjax")]),
        heading_tags: BTreeMap::from([(Name::from("section"), "h2".to_string())]),
        root_heading_level: 2,
    };
    assert_eq!(read, built);

    let read = option::<LatexRecomposedRenderOptions>(
        "latex-recomposed",
        json!({
            "unicode": "replace",
            "unicode_overrides": { "é": "\\'e" },
            "drop_comments": true,
            "skip_packages": ["hyperref"],
            "extra_packages": [{ "id": "my.xcolor", "package": "xcolor", "options": ["dvipsnames"], "docs": "" }],
            "extra_preamble": { "before_packages": "%a", "after_packages": "%b", "at_end": "%c" },
            "bare_names": ["emph"],
            "robust_names": ["textbf"],
            "style_stem": "mystyle",
            "style_file": "always",
            "heading_hook": false,
            "label_hooks": false,
            "drop_math_labels_without_refs": true,
            "resolved_proof_refs": true,
        }),
    );
    let built = LatexRecomposedRenderOptions {
        unicode: UnicodeMode::Replace,
        unicode_overrides: BTreeMap::from([('é', "\\'e".to_string())]),
        drop_comments: true,
        skip_packages: BTreeSet::from(["hyperref".to_string()]),
        extra_packages: vec![LatexPackageDef {
            id: Name::from("my.xcolor"),
            package: "xcolor".into(),
            options: vec!["dvipsnames".into()],
            docs: String::new(),
            engines: EngineSet::ALL,
        }],
        extra_preamble: ExtraPreamble {
            before_packages: "%a".into(),
            after_packages: "%b".into(),
            at_end: "%c".into(),
        },
        bare_names: BTreeSet::from(["emph".to_string()]),
        robust_names: BTreeSet::from(["textbf".to_string()]),
        style_stem: "mystyle".into(),
        style_file: StyleFile::Always,
        heading_hook: false,
        label_hooks: false,
        drop_math_labels_without_refs: true,
        resolved_proof_refs: true,
    };
    assert_eq!(read, built);
}

#[test]
fn empty_data_reads_as_every_default() {
    same(&config::<BlocksConfig>("blocks", json!({})), &BlocksConfig::default());
    same(&config::<UserdefsConfig>("userdefs", json!({})), &UserdefsConfig::default());
    same(&config::<HeadingsConfig>("headings", json!({})), &HeadingsConfig::default());
    same(&config::<NumberingConfig>("numbering", json!({})), &NumberingConfig::default());
    same(&config::<MathConfig>("math", json!({})), &MathConfig::default());
    same(&config::<RefsConfig>("refs", json!({})), &RefsConfig::default());
    same(&config::<ListsConfig>("lists", json!({})), &ListsConfig::default());
    same(&config::<XnotesConfig>("xnotes", json!({})), &XnotesConfig::default());
    same(&config::<DeftermConfig>("defterm", json!({})), &DeftermConfig::default());
    same(&config::<TheoremsConfig>("theorems", json!({})), &TheoremsConfig::default());
    same(&config::<QuoteConfig>("quote", json!({})), &QuoteConfig::default());
    same(&config::<AnnotationsConfig>("annotations", json!({})), &AnnotationsConfig::default());
    same(&config::<GraphicsConfig>("graphics", json!({})), &GraphicsConfig::default());
    same(&config::<FloatsConfig>("floats", json!({})), &FloatsConfig::default());
    same(&config::<CellsConfig>("cells", json!({})), &CellsConfig::default());
    same(&config::<CitationsConfig>("citations", json!({})), &CitationsConfig::default());
    // An option type none of whose keys is written is not set at all, so the
    // run reads its default.
    for feature in ["pieces", "text", "html", "latex-recomposed", "refs", "math", "annotations"] {
        let settings = read(feature, json!({}));
        assert!(settings.render_options().is_empty(), "{feature}");
    }
    assert!(read("numbering", json!({})).document_options().is_empty());
}

#[test]
fn each_feature_states_its_own_declaration_and_the_registry_declares_its_types() {
    // A declaration names the feature that states it, and declares keys.
    for feature in stdcore::package(StdcoreConfig::new()).features {
        if let Some(decl) = feature.settings() {
            assert_eq!(decl.feature(), &feature.info().name);
            assert!(decl.keys().next().is_some(), "{}", feature.info().name);
        }
    }
    // The factory lists what the features state.
    for feature in factory().features() {
        if let Some(decl) = &feature.settings {
            assert_eq!(decl.feature(), &feature.name);
        }
    }
    // The bindings: a configuration's keys are bound with the registry, the
    // option types' as they declare.
    let refs = decl("refs");
    assert_eq!(refs.key("join").map(|key| key.bind), Some(Bind::Registry));
    assert_eq!(refs.key("unresolved").map(|key| key.bind), Some(Bind::Render));
    assert_eq!(decl("numbering").key("counters").map(|key| key.bind), Some(Bind::Either));
    // The registry build declares the option types of the declaration, with
    // their bindings, although no feature's registration names them.
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
    let declared = |key: OptionKey| registry.option_decl(&key).map(|decl| (decl.owner.to_string(), decl.bind));
    assert_eq!(declared(OptionKey::of::<RefsConfig>()), Some(("refs".into(), Bind::Registry)));
    assert_eq!(declared(OptionKey::of::<RefsRenderOptions>()), Some(("refs".into(), Bind::Render)));
    assert_eq!(declared(OptionKey::of::<NumberingOptions>()), Some(("numbering".into(), Bind::Either)));
}

#[test]
fn each_collection_of_named_things_is_marked() {
    let marked = [
        ("userdefs", "macros"),
        ("userdefs", "environments"),
        ("userdefs", "specials"),
        ("xnotes", "kinds"),
        ("floats", "types"),
        ("annotations", "annotators"),
        ("headings", "headings"),
        ("headings", "commands"),
        ("numbering", "styles"),
        ("theorems", "classes"),
        ("theorems", "kinds"),
        ("quote", "environments"),
        ("lists", "environments"),
        ("graphics", "strategy"),
        ("graphics", "convert"),
        ("graphics", "magnification"),
    ];
    for (feature, key) in marked {
        let decl = decl(feature);
        assert!(decl.key(key).is_some(), "{feature}.{key}");
        assert!(
            decl.mark_at(key).is_some_and(|mark| mark.merge_entries),
            "{feature}.{key} is merged entry by entry"
        );
    }
    let merged: usize = factory()
        .features()
        .iter()
        .filter_map(|feature| feature.settings.as_ref())
        .map(|decl| decl.marks().iter().filter(|mark| mark.merge_entries).count())
        .sum();
    assert_eq!(merged, marked.len());
}

/// The data of `feature`'s settings holding, for each of its keys merged entry
/// by entry, the default entries its declaration gives.
fn default_entries(feature: &str) -> Value {
    let decl = decl(feature);
    let mut data = serde_json::Map::new();
    for mark in decl.marks().iter().filter(|mark| mark.merge_entries) {
        let entries = decl.default_entries(&mark.path).expect("marked");
        data.insert(mark.path.clone(), Value::Object(entries.clone()));
    }
    Value::Object(data)
}

#[test]
fn the_default_entries_of_each_marked_key_read_as_the_feature_s_default() {
    same(&config::<UserdefsConfig>("userdefs", default_entries("userdefs")), &UserdefsConfig::default());
    same(&config::<HeadingsConfig>("headings", default_entries("headings")), &HeadingsConfig::default());
    same(&config::<NumberingConfig>("numbering", default_entries("numbering")), &NumberingConfig::default());
    same(&config::<ListsConfig>("lists", default_entries("lists")), &ListsConfig::default());
    same(&config::<XnotesConfig>("xnotes", default_entries("xnotes")), &XnotesConfig::default());
    same(&config::<TheoremsConfig>("theorems", default_entries("theorems")), &TheoremsConfig::default());
    same(&config::<QuoteConfig>("quote", default_entries("quote")), &QuoteConfig::default());
    same(&config::<AnnotationsConfig>("annotations", default_entries("annotations")), &AnnotationsConfig::default());
    same(&config::<FloatsConfig>("floats", default_entries("floats")), &FloatsConfig::default());
    // A key that is not marked has no default entries.
    assert!(decl("text").default_entries("width").is_none());
    assert!(decl("floats").default_entries("types").is_some());
}

#[test]
#[should_panic(expected = "marks `kinds.words` as merged entry by entry, and only a setting itself can be marked so, not a path below one")]
fn a_path_below_a_key_cannot_be_merged_entry_by_entry() {
    let _ = SettingsDecl::new("xnotes")
        .config::<XnotesConfig>("The notes.")
        .merge_entries("kinds.words", || Ok(Value::Object(serde_json::Map::new())));
}

#[test]
#[should_panic(expected = "the default entries of `xnotes.kinds` cannot be written as data: no data")]
fn default_entries_that_cannot_be_written_are_a_fault_of_the_declaration() {
    let _ = SettingsDecl::new("xnotes")
        .config::<XnotesConfig>("The notes.")
        .merge_entries("kinds", || Err(String::from("no data")));
}

#[test]
#[should_panic(expected = "marks `kinds.*` as merged entry by entry, and only a setting itself can be marked so, not a path below one")]
fn every_entry_of_a_key_cannot_be_merged_entry_by_entry() {
    let _ = SettingsDecl::new("xnotes")
        .config::<XnotesConfig>("The notes.")
        .merge_entries("kinds.*", || Ok(Value::Object(serde_json::Map::new())));
}

// ---- test 2: problems are named by the key's path -------------------------

#[test]
fn an_undeclared_key_is_named_by_its_path() {
    let errors = decl("html").read(json!({ "heading_tag": {} }).as_object().unwrap()).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, "html.heading_tag");
    assert_eq!(errors[0].kind, SettingsErrorKind::UnknownKey);
}

#[test]
fn a_value_of_the_wrong_shape_is_named_by_its_path() {
    let errors = decl("text").read(json!({ "width": "wide" }).as_object().unwrap()).unwrap_err();
    assert_eq!(errors[0].path, "text.width");
    assert_eq!(errors[0].kind, SettingsErrorKind::InvalidValue);

    let errors = decl("html")
        .read(json!({ "heading_tags": { "section": 2 } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "html.heading_tags.section");

    // Inside a collection of named things, the problem stands at its entry.
    let errors = decl("xnotes")
        .read(json!({ "kinds": { "endnote": { "numbered": "yes" } } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "xnotes.kinds.endnote");
    let errors = decl("xnotes")
        .read(json!({ "kinds": { "endnote": { "no_such_field": 1 } } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "xnotes.kinds.endnote");

    // A key below a type that refuses unknown fields.
    let errors = decl("refs")
        .read(json!({ "join": { "nand": "x" } }).as_object().unwrap())
        .unwrap_err();
    assert!(errors[0].path.starts_with("refs.join"), "{}", errors[0].path);

    // An argument code no form answers to.
    let errors = decl("userdefs")
        .read(json!({ "macros": { "hi": { "args": ["q"] } } }).as_object().unwrap())
        .unwrap_err();
    assert_eq!(errors[0].path, "userdefs.macros.hi");
}

#[test]
fn every_problem_is_reported() {
    let errors = decl("text")
        .read(json!({ "widht": 3, "tab_width": "four", "colour": 1 }).as_object().unwrap())
        .unwrap_err();
    let paths: BTreeSet<&str> = errors.iter().map(|error| error.path.as_str()).collect();
    assert_eq!(paths, BTreeSet::from(["text.widht", "text.colour", "text.tab_width"]));
}

// ---- a validator for the subset of JSON Schema the schemas use ------------

#[path = "support/json_schema.rs"]
mod validator;

/// Whether the schema of `feature`'s settings accepts `data`.
fn schema_accepts(feature: &str, data: &Value) -> bool {
    let schema = decl(feature).json_schema();
    validator::valid(&schema, &schema, data)
}

// ---- test 3: the schema ----------------------------------------------------

mod schema {
    use super::*;

    fn schemas() -> serde_json::Map<String, Value> {
        factory()
            .features()
            .iter()
            .filter_map(|feature| feature.settings.as_ref())
            .map(|decl| (decl.feature().to_string(), decl.json_schema()))
            .collect()
    }

    #[test]
    fn every_declared_key_stands_in_the_schema_with_its_documentation_and_binding() {
        for feature in factory().features() {
            let Some(decl) = &feature.settings else { continue };
            let schema = decl.json_schema();
            assert_eq!(schema["additionalProperties"], json!(false));
            for key in decl.keys() {
                let property = &schema["properties"][key.name.as_str()];
                assert_eq!(property["x-flm-bind"], json!(key.bind.label()), "{}.{}", feature.name, key.name);
                assert!(
                    property["description"].as_str().is_some_and(|text| !text.is_empty()),
                    "{}.{} is documented",
                    feature.name,
                    key.name
                );
            }
        }
    }

    // Data the schema accepts is read: every reading of test 1 goes through
    // `read`, which checks the data against the feature's schema first.

    #[test]
    fn an_entry_the_reader_refuses_the_schema_refuses_too() {
        let words = json!({ "base": { "one": "aside", "other": "asides" } });
        let refused = [
            // A required field left out.
            ("headings", json!({ "headings": { "part": { "run_in": true } } })),
            ("headings", json!({ "commands": { "part": {} } })),
            ("floats", json!({ "types": { "chart": {} } })),
            ("xnotes", json!({ "kinds": { "aside": { "commands": ["aside"] } } })),
            ("lists", json!({ "environments": { "steps": {} } })),
            ("theorems", json!({ "kinds": { "claim": {} } })),
            ("quote", json!({ "environments": { "verse": {} } })),
            // The name written inside the entry.
            ("headings", json!({ "headings": { "part": { "level": 0, "name": "part" } } })),
            ("headings", json!({ "commands": { "part": { "heading": "part", "name": "part" } } })),
            ("floats", json!({ "types": { "chart": { "handlers": [], "name": "chart" } } })),
            ("xnotes", json!({ "kinds": { "aside": { "words": words, "name": "aside" } } })),
            ("annotations", json!({ "annotators": { "phf": { "command": "phf" } } })),
            ("numbering", json!({ "styles": { "my.style": { "name": "my.style" } } })),
            ("citations", json!({ "style": { "name": "my.style" } })),
            ("userdefs", json!({ "macros": { "hi": { "name": "hi" } } })),
            ("userdefs", json!({ "environments": { "hi": { "kind": "environment" } } })),
            ("userdefs", json!({ "specials": { "~~": { "modes": "math" } } })),
            ("theorems", json!({ "classes": { "plain": { "name": "plain" } } })),
            ("lists", json!({ "environments": { "steps": { "name": "steps" } } })),
            ("quote", json!({ "environments": { "verse": { "name": "verse" } } })),
            // A content by mode that gives no mode.
            ("userdefs", json!({ "macros": { "hi": { "content": {} } } })),
            // A field of the package request that the settings form does not
            // have: the engines it applies to (every engine, for an author's).
            (
                "latex-recomposed",
                json!({
                    "extra_packages": [
                        { "id": "my.x", "package": "x", "options": [], "docs": "", "engines": "all" },
                    ],
                }),
            ),
        ];
        for (feature, data) in refused {
            assert!(decl(feature).read(data.as_object().unwrap()).is_err(), "`{feature}` reads {data}");
            assert!(!schema_accepts(feature, &data), "the schema of `{feature}` accepts {data}");
        }
    }

    /// Whether the path `segments` names a place in `schema`: a property, `*`
    /// for the values of a map or the items of a list, or the one key of a
    /// variant written as a map.
    fn names_a_place(root: &Value, schema: &Value, segments: &[&str]) -> bool {
        let schema = match schema.get("$ref").and_then(Value::as_str) {
            Some(reference) => &root["$defs"][reference.trim_start_matches("#/$defs/")],
            None => schema,
        };
        let Some((first, rest)) = segments.split_first() else { return true };
        let branches = ["anyOf", "oneOf"]
            .iter()
            .filter_map(|keyword| schema.get(*keyword).and_then(Value::as_array))
            .flatten()
            .any(|branch| names_a_place(root, branch, segments));
        let below = if *first == "*" {
            [schema.get("additionalProperties"), schema.get("items")]
                .into_iter()
                .flatten()
                .any(|inner| inner.is_object() && names_a_place(root, inner, rest))
        } else {
            schema.get("properties").and_then(|properties| properties.get(*first)).is_some_and(|inner| {
                names_a_place(root, inner, rest)
            })
        };
        branches || below
    }

    #[test]
    fn every_mark_names_a_key_of_the_schema() {
        for feature in factory().features() {
            let Some(decl) = &feature.settings else { continue };
            let schema = decl.json_schema();
            for mark in decl.marks() {
                let segments: Vec<&str> = mark.path.split('.').collect();
                assert!(names_a_place(&schema, &schema, &segments), "{}.{}", feature.name, mark.path);
            }
        }
    }

    /// Every description of `schema`, the data under `default` left out.
    fn descriptions(schema: &Value, found: &mut Vec<String>) {
        match schema {
            Value::Object(object) => {
                for (keyword, value) in object {
                    match (keyword.as_str(), value) {
                        ("description", Value::String(text)) => found.push(text.clone()),
                        ("default" | "const" | "enum", _) => {}
                        (_, value) => descriptions(value, found),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| descriptions(item, found)),
            _ => {}
        }
    }

    #[test]
    fn every_description_is_plain_text() {
        let mut found = Vec::new();
        descriptions(&Value::Object(schemas()), &mut found);
        assert!(found.len() > 100);
        for text in found {
            assert!(!text.contains("]("), "a link: {text}");
            assert!(!text.lines().any(|line| line.trim_start().starts_with("```")), "a code block: {text}");
            assert!(!text.lines().any(|line| line.starts_with("# ")), "a section: {text}");
            assert!(!text.is_empty() && text == text.trim(), "{text:?}");
        }
        // A link is written as its words.
        let schema = decl("blocks").json_schema();
        let ligatures = schema["properties"]["ligature_unicode_quotes"]["description"].as_str().unwrap();
        assert!(ligatures.contains("only when `auto_unicode_quotes` is"), "{ligatures}");
    }

    #[test]
    fn a_key_merged_entry_by_entry_has_its_default_entries_as_its_default() {
        let schema = decl("floats").json_schema();
        let default = &schema["properties"]["types"]["default"];
        assert_eq!(default.as_object().unwrap().keys().collect::<Vec<_>>(), ["figure", "table"]);
        assert!(schema_accepts("floats", &json!({ "types": default })));
    }

    #[test]
    fn each_argument_code_a_file_may_write_is_shown_with_its_parser_s_phrase() {
        let schema = decl("userdefs").json_schema();
        let text = serde_json::to_string(&schema).unwrap();
        for code in ["{", "m", "[", "o", "*", "s", "v", "AnyDelimited", "AnyDelimitedOptional", "BracedOnly"] {
            let phrase = flm_core::lang::mint::arg_def_from_code(code).unwrap().kind.description();
            let shown = json!({ "const": code, "description": phrase });
            assert!(text.contains(&shown.to_string()), "`{code}` with {phrase:?}");
        }
        // The schema admits each code and refuses one no parser reads.
        for code in ["{", "o", "*"] {
            assert!(schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": [code] } } })), "{code}");
            assert!(schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": [{ "code": code }] } } })), "{code}");
        }
        assert!(!schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": ["q"] } } })));
        // A code that takes characters is matched by its form, written whole.
        for code in ["t!", "r()", "d<>", "v||", "e{_^}"] {
            assert!(schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": [code] } } })), "{code}");
        }
        for code in ["t", "r(", "r()x", "e{}", "e{_"] {
            assert!(!schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": [code] } } })), "{code}");
        }
        // A list written as one text holds codes of one character.
        assert!(schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": "[{*v" } } })));
        assert!(!schema_accepts("userdefs", &json!({ "macros": { "hi": { "args": "{q" } } })));
    }

    // The description is the documentation text the declaration gives.
    #[cfg(feature = "docs")]
    #[test]
    fn each_feature_s_schema_is_described_by_its_declaration() {
        for feature in factory().features() {
            let Some(decl) = &feature.settings else { continue };
            let schema = decl.json_schema();
            assert!(schema["description"].as_str().is_some_and(|text| !text.is_empty()), "{}", feature.name);
        }
    }

    // The schema holds the documentation texts the declarations give.
    #[cfg(feature = "docs")]
    #[test]
    fn the_schema_of_stdcore_is_a_golden() {
        let text = serde_json::to_string_pretty(&Value::Object(schemas())).unwrap() + "\n";
        flm_core::dump::golden::assert_text_golden(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens/stdcore/settings-schema.json"),
            &text,
        );
    }
}

// ---- test 4: the factory of stdcore ----------------------------------------

#[test]
fn the_factory_states_its_name_its_features_in_order_and_no_package_used() {
    let factory = factory();
    assert_eq!(factory.name(), stdcore::PACKAGE);
    assert!(factory.used().is_empty());
    let names: Vec<String> = factory.features().iter().map(|feature| feature.name.to_string()).collect();
    let package: Vec<String> = stdcore::package(StdcoreConfig::new())
        .features
        .iter()
        .map(|feature| feature.info().name.to_string())
        .collect();
    assert_eq!(names, package);
}

fn digest(entry: PackageEntry) -> u64 {
    let mut builder = RegistryBuilder::new();
    builder.use_package(entry);
    builder.build(&RegistryInputs::new()).expect("the registry builds").digest()
}

#[test]
fn built_from_empty_settings_the_factory_gives_the_package() {
    let from_factory = factory().build(&PackageSettings::new()).unwrap();
    assert_eq!(digest(from_factory), digest(stdcore::package(StdcoreConfig::new())));

    // Settings read from empty data are the defaults too.
    let factory = factory();
    let mut settings = PackageSettings::new();
    for feature in factory.features() {
        if let Some(decl) = &feature.settings {
            settings.insert(decl.read(&serde_json::Map::new()).unwrap());
        }
    }
    assert_eq!(
        digest(factory.build(&settings).unwrap()),
        digest(stdcore::package(StdcoreConfig::new()))
    );
}

#[test]
fn a_configuration_read_from_data_reaches_the_package() {
    let factory = factory();
    let mut settings = PackageSettings::new();
    settings.insert(read("math", json!({ "numbered_equations": false })));
    let expected = stdcore::package(StdcoreConfig::new().math(MathConfig { numbered_equations: false }));
    assert_eq!(digest(factory.build(&settings).unwrap()), digest(expected));
}

#[test]
fn a_feature_switched_off_is_absent() {
    let mut settings = PackageSettings::new();
    settings.switch_off("annotations");
    let entry = factory().build(&settings).unwrap();
    assert!(entry.features.iter().all(|feature| feature.info().name != "annotations"));
    assert_eq!(entry.features.len(), stdcore::package(StdcoreConfig::new()).features.len() - 1);
}

// ---- test 5: the catalog ---------------------------------------------------

struct Asked {
    name: &'static str,
    asked: std::sync::Mutex<Vec<String>>,
}

impl PackageResolver for Asked {
    fn resolve(&self, name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
        self.asked.lock().unwrap().push(name.to_string());
        if name == self.name {
            let owned = self.name;
            let factory = PackageFactory::new(owned, move |_| Ok(PackageEntry::new(owned)))
                .map_err(|error| vec![ResolveError::new(error.to_string())])?;
            Ok(Some(factory))
        } else {
            Ok(None)
        }
    }
}

#[test]
fn the_catalog_answers_a_name_it_holds_and_asks_its_resolvers_in_order() {
    let first = Arc::new(Asked { name: "one", asked: Default::default() });
    let second = Arc::new(Asked { name: "two", asked: Default::default() });
    let mut catalog = PackageCatalog::new();
    catalog.add_compiled(factory());
    catalog.add_resolver(first.clone());
    catalog.add_resolver(second.clone());

    assert_eq!(catalog.resolve("stdcore").unwrap().unwrap().name(), "stdcore");
    assert!(first.asked.lock().unwrap().is_empty(), "a compiled package asks no resolver");

    assert_eq!(catalog.resolve("two").unwrap().unwrap().name(), "two");
    assert_eq!(*first.asked.lock().unwrap(), ["two"]);
    assert_eq!(*second.asked.lock().unwrap(), ["two"]);

    assert_eq!(catalog.resolve("one").unwrap().unwrap().name(), "one");
    assert_eq!(*second.asked.lock().unwrap(), ["two"], "the first answer stops the search");

    assert!(catalog.resolve("three").unwrap().is_none());
    assert_eq!(catalog.compiled_names().map(|name| name.as_str()).collect::<Vec<_>>(), ["stdcore"]);
}

#[test]
fn a_resolver_failure_stops_the_search() {
    struct Failing;
    impl PackageResolver for Failing {
        fn resolve(&self, _name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
            Err(vec![ResolveError::new("unreadable")])
        }
    }
    let later = Arc::new(Asked { name: "x", asked: Default::default() });
    let mut catalog = PackageCatalog::new();
    catalog.add_resolver(Arc::new(Failing));
    catalog.add_resolver(later.clone());
    assert!(catalog.resolve("x").is_err());
    assert!(later.asked.lock().unwrap().is_empty());
}

fn plain(name: &'static str, uses: &[&'static str]) -> PackageFactory {
    let mut factory = PackageFactory::new(name, move |_| Ok(PackageEntry::new(name))).unwrap();
    for used in uses {
        factory = factory.uses(*used);
    }
    factory
}

#[test]
fn the_catalog_orders_the_packages_each_after_those_it_uses_and_each_once() {
    let mut catalog = PackageCatalog::new();
    catalog.add_compiled(plain("base", &[]));
    catalog.add_compiled(plain("left", &["base"]));
    catalog.add_compiled(plain("right", &["base", "left"]));
    let found = catalog.resolve_all(&["right", "left", "base"]).unwrap();
    let names: Vec<&str> = found.iter().map(|factory| factory.name().as_str()).collect();
    assert_eq!(names, ["base", "left", "right"]);
}

#[test]
fn the_catalog_walks_packages_that_use_each_other_and_reports_every_problem() {
    let mut catalog = PackageCatalog::new();
    catalog.add_compiled(plain("base", &[]));
    catalog.add_compiled(plain("a", &["b"]));
    catalog.add_compiled(plain("b", &["base", "a"]));
    catalog.add_compiled(plain("self", &["self", "base"]));
    let found = catalog.resolve_all(&["a", "self"]).unwrap();
    let names: Vec<&str> = found.iter().map(|factory| factory.name().as_str()).collect();
    assert_eq!(names, ["base", "b", "a", "self"]);
    let errors = catalog.resolve_all(&["nosuch", "a", "absent"]).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(matches!(errors[0].kind, flm_core::core::registry::UseErrorKind::Unknown { .. }));
    assert_eq!((errors[0].index, errors[0].package.as_str()), (0, "nosuch"));
    assert_eq!((errors[1].index, errors[1].package.as_str()), (2, "absent"));
}

#[test]
fn the_catalog_takes_two_names_of_one_factory_for_one_package() {
    struct Spellings;
    impl PackageResolver for Spellings {
        fn resolve(&self, name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
            Ok(["x", "./x"].contains(&name).then(|| plain("the-file-x", &["./x"])))
        }
    }
    let mut catalog = PackageCatalog::new();
    catalog.add_resolver(Arc::new(Spellings));
    let found = catalog.resolve_all(&["x", "./x"]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name(), "the-file-x");
}

#[test]
fn the_catalog_reports_a_resolver_s_failure_for_the_package() {
    struct Failing;
    impl PackageResolver for Failing {
        fn resolve(&self, _name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
            Err(vec![ResolveError::new("unreadable")])
        }
    }
    let mut catalog = PackageCatalog::new();
    catalog.add_compiled(plain("top", &["found-elsewhere"]));
    catalog.add_resolver(Arc::new(Failing));
    let errors = catalog.resolve_all(&["top"]).unwrap_err();
    assert_eq!(errors[0].package.as_str(), "found-elsewhere");
    assert!(matches!(errors[0].kind, flm_core::core::registry::UseErrorKind::Resolver(_)));
}
