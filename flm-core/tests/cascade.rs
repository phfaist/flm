//! The settings cascade (`flm_core::core::settings::cascade`): the rule of the ranks,
//! peers and reach, the entry-by-entry merge, the switch-off rule, workflow fields and
//! workflow settings, path resolution, the parsing settings and the settings in effect,
//! over a test origin and the standard package's declarations. Moved from
//! `flm-main/src/settings.rs` with the cascade; an integration test because the module
//! layering keeps `core` from naming `stdcore`.

use std::fmt;
use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, PackageFactory};
use flm_core::core::settings::SettingsDecl;
use flm_core::core::settings::cascade::{Cascade, CascadeOrigin, Problem, Reach, SettingValue};
use flm_core::model::names::Name;
use flm_core::model::options::DataValue;
use flm_core::stdcore::backends::html::HtmlRenderOptions;
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
use flm_core::stdcore::backends::text::TextRenderOptions;
use flm_core::stdcore::factory;
use flm_core::stdcore::features::numbering::NumberingOptions;
use flm_core::stdcore::features::xnotes::XnotesConfig;
use serde_json::{Map, Value};

/// The origin of the tests: five sources, from the lowest rank to the highest, each
/// with the line it is written at where it is a file. The packages and the
/// workflow's settings are peers; a document's front matter is the higher source a
/// person writes in; a file with a folder resolves a path against that folder.
#[derive(Debug, Clone, PartialEq)]
enum TestOrigin {
    Defaults,
    Package(&'static str, u32),
    Document(u32),
    Workflow(&'static str, &'static str, u32),
    CommandLine(usize),
}

use TestOrigin::{CommandLine, Defaults, Document, Package, Workflow};

impl fmt::Display for TestOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Defaults => f.write_str("the program's defaults"),
            Package(file, line) | Workflow(_, file, line) => write!(f, "{file}:{line}:1"),
            Document(line) => write!(f, "d.flm:{line}:1"),
            CommandLine(index) => write!(f, "the option {index}"),
        }
    }
}

impl CascadeOrigin for TestOrigin {
    fn rank(&self) -> u32 {
        match self {
            Defaults => 0,
            Package(..) => 1,
            Document(_) => 2,
            Workflow(..) => 3,
            CommandLine(_) => 4,
        }
    }

    fn peer(&self) -> Option<&str> {
        match self {
            Package(file, _) | Workflow(_, file, _) => Some(file),
            _ => None,
        }
    }

    fn workflow(&self) -> Option<&str> {
        match self {
            Workflow(workflow, ..) => Some(workflow),
            _ => None,
        }
    }

    fn higher_source(&self) -> Option<&str> {
        match self {
            Package(..) => Some("the document's front matter"),
            _ => None,
        }
    }

    fn resolve_path(&self, path: &str) -> Option<String> {
        let file = match self {
            Package(file, _) | Workflow(_, file, _) => *file,
            _ => return None,
        };
        let (folder, _) = file.rsplit_once('/')?;
        if path.starts_with('/') { None } else { Some(format!("{folder}/{path}")) }
    }
}

type Settings = Cascade<TestOrigin>;

#[test]
fn a_higher_rank_replaces_a_value_whole_and_a_lower_one_does_not() {
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(60), Defaults);
    settings.set("text", "width", Value::from(70), Document(1));
    settings.set("text", "width", Value::from(80), CommandLine(1));
    settings.set("text", "width", Value::from(90), Document(1));
    let value = settings.value("text", "width").unwrap();
    assert_eq!(value.value, Value::from(80));
    assert_eq!(value.origin.rank(), 4);
    settings.set("html", "heading_tags", serde_json::json!({"section": "h2"}), Defaults);
    settings.set("html", "heading_tags", serde_json::json!({"subsection": "h3"}), Document(1));
    assert_eq!(settings.value("html", "heading_tags").unwrap().value, serde_json::json!({"subsection": "h3"}));
}

#[test]
fn keys_of_each_binding_are_read_into_their_places() {
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(50), CommandLine(1));
    let counters = serde_json::json!({ "flm.headings.section": { "initial": 0 } });
    settings.set("numbering", "counters", counters, CommandLine(2));
    settings.set("xnotes", "kinds", serde_json::json!({ "footnote": false }), CommandLine(3));
    let factory = factory().unwrap();
    let read = settings.read(&[&factory]).unwrap();
    assert_eq!(read.render.get::<TextRenderOptions>().unwrap().width, Some(50));
    let package = &read.packages[factory.name()];
    assert!(package.config::<XnotesConfig>("xnotes").unwrap().kinds.is_empty());
    let counters = &read.document.get::<NumberingOptions>().unwrap().counters;
    assert_eq!(counters.values().next().unwrap().initial, Some(0));
}

#[test]
fn a_problem_is_placed_at_the_value_at_fault() {
    let mut settings = Settings::new();
    settings.set("text", "wdth", Value::from(1), Document(4));
    settings.set("nosuch", "x", Value::from(1), Defaults);
    let factory = factory().unwrap();
    let problems = settings.read(&[&factory]).unwrap_err().errors;
    assert_eq!(problems.len(), 2);
    assert!(problems.iter().any(|problem| problem.origin == Some(Document(4))));
}

#[test]
fn false_for_a_feature_no_package_has_is_a_warning_each_time_and_settings_for_it_a_problem() {
    let factory = factory().unwrap();
    let warning = "`nosuch` is switched off, but no package in use has a feature `nosuch`";
    // `false` alone: accepted, one warning for each `false` written, at its origin.
    let mut settings = Settings::new();
    settings.switch_off("nosuch", Package("a.yaml", 2));
    settings.switch_off("nosuch", Document(3));
    let read = settings.read(&[&factory]).unwrap();
    let warnings: Vec<(Option<TestOrigin>, &str)> =
        read.warnings.iter().map(|problem| (problem.origin.clone(), problem.detail.as_str())).collect();
    assert_eq!(warnings, [(Some(Package("a.yaml", 2)), warning), (Some(Document(3)), warning)]);
    // A feature that a package has gives no warning when switched off.
    let mut known = Settings::new();
    known.switch_off("text", Document(1));
    assert!(known.read(&[&factory]).unwrap().warnings.is_empty());
    // Settings for it, a key or an empty map, are a problem at the settings written.
    let unknown = "`nosuch` must be the name of a feature of a package the document uses";
    let mut keyed = Settings::new();
    keyed.switch_off("nosuch", Document(3));
    keyed.set("nosuch", "x", Value::from(1), Document(4));
    let failure = keyed.read(&[&factory]).unwrap_err();
    assert_eq!(failure.errors, [Problem { origin: Some(Document(4)), detail: unknown.to_string() }]);
    // The warning of a `false` comes with the problems.
    assert_eq!(failure.warnings, [Problem { origin: Some(Document(3)), detail: warning.to_string() }]);
    let mut mapped = Settings::new();
    mapped.switch_on("nosuch", Package("a.yaml", 5));
    let problems = mapped.read(&[&factory]).unwrap_err().errors;
    assert_eq!(problems, [Problem { origin: Some(Package("a.yaml", 5)), detail: unknown.to_string() }]);
    // A problem with another feature's settings: the warning comes with it.
    let mut beside = Settings::new();
    beside.switch_off("nosuch", Document(3));
    beside.set("text", "wdth", Value::from(1), Document(4));
    let failure = beside.read(&[&factory]).unwrap_err();
    assert_eq!(failure.errors.len(), 1, "{:?}", failure.errors);
    assert_eq!(failure.errors[0].origin, Some(Document(4)));
    assert_eq!(failure.warnings, [Problem { origin: Some(Document(3)), detail: warning.to_string() }]);
}

#[test]
fn a_workflow_outranks_the_front_matter_and_the_command_line_outranks_it() {
    let width = |settings: &Settings| {
        let factory = factory().unwrap();
        settings.read(&[&factory]).unwrap().render.get::<TextRenderOptions>().unwrap().width
    };
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(70), Document(1));
    settings.set("text", "width", Value::from(60), Workflow("w", "p.yaml", 1));
    assert_eq!(width(&settings), Some(60));
    settings.set("text", "width", Value::from(50), CommandLine(1));
    assert_eq!(width(&settings), Some(50));
}

#[test]
fn a_workflow_sets_only_keys_bound_per_render() {
    let mut settings = Settings::new();
    settings.set("xnotes", "kinds", serde_json::json!({}), Workflow("w", "p.yaml", 2));
    let factory = factory().unwrap();
    let problems = settings.read(&[&factory]).unwrap_err().errors;
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].origin, Some(Workflow("w", "p.yaml", 2)));
    assert!(problems[0].detail.contains("the workflow `w` must not set `xnotes.kinds`"), "{}", problems[0].detail);
}

#[test]
fn the_keys_of_a_feature_switched_off_are_not_read() {
    let mut settings = Settings::new();
    settings.switch_off("text", Document(1));
    settings.set("text", "width", Value::from(30), CommandLine(1));
    settings.set("text", "wdth", Value::from(1), Defaults);
    let factory = factory().unwrap();
    let read = settings.read(&[&factory]).unwrap();
    assert!(read.render.get::<TextRenderOptions>().is_none());
    assert!(read.packages[factory.name()].is_off("text"));
}

#[test]
fn a_package_replaces_a_default_and_the_front_matter_replaces_a_package() {
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(60), Defaults);
    settings.set("text", "width", Value::from(70), Package("a.yaml", 2));
    assert_eq!(settings.value("text", "width").unwrap().value, Value::from(70));
    settings.set("text", "width", Value::from(80), Document(3));
    settings.set("text", "width", Value::from(90), Package("b.yaml", 2));
    let value = settings.value("text", "width").unwrap();
    assert_eq!((value.value, value.origin), (Value::from(80), Document(3)));
}

#[test]
fn peers_combine_agree_or_disagree_and_a_higher_source_settles_them() {
    let factory = factory().unwrap();
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(70), Package("a.yaml", 2));
    settings.set("text", "width", Value::from(70), Package("b.yaml", 5));
    settings.set("numbering", "counters", serde_json::json!({}), Package("b.yaml", 6));
    assert!(settings.read(&[&factory]).is_ok());
    settings.set("text", "width", Value::from(71), Package("b.yaml", 7));
    let problems = settings.read(&[&factory]).unwrap_err().errors;
    assert_eq!(problems.len(), 1);
    assert!(problems[0].detail.contains("a.yaml:2:1") && problems[0].detail.contains("b.yaml:7:1"), "{}", problems[0].detail);
    settings.set("text", "width", Value::from(50), Document(3));
    assert!(settings.read(&[&factory]).is_ok());
}

#[test]
fn the_source_of_the_highest_rank_switches_a_feature_on_or_off() {
    let factory = factory().unwrap();
    let mut settings = Settings::new();
    settings.switch_off("text", Package("a.yaml", 2));
    assert!(settings.switched_off("text").is_some());
    settings.switch_on("text", Document(2));
    assert!(settings.switched_off("text").is_none());
    assert!(!settings.read(&[&factory]).unwrap().packages[factory.name()].is_off("text"));
    let mut peers = Settings::new();
    peers.switch_off("text", Package("a.yaml", 2));
    peers.switch_on("text", Package("b.yaml", 4));
    assert_eq!(peers.read(&[&factory]).unwrap_err().errors.len(), 1);
}

#[test]
fn the_fields_packages_write_for_a_workflow_are_peers() {
    let factory = factory().unwrap();
    let docs = |text: &str, origin: TestOrigin| vec![(String::from("docs"), Value::from(text), origin)];
    let mut settings = Settings::new();
    settings.add_workflow_fields("text", docs("A", Package("a.yaml", 2)));
    settings.add_workflow_fields("text", docs("A", Package("b.yaml", 3)));
    assert!(settings.read(&[&factory]).is_ok());
    assert_eq!(settings.workflow_field("text", "docs").unwrap().origin, Package("a.yaml", 2));
    settings.add_workflow_fields("text", docs("B", Package("c.yaml", 4)));
    let problems = settings.read(&[&factory]).unwrap_err().errors;
    assert_eq!(problems.len(), 1);
    assert!(problems[0].detail.contains("a.yaml:2:1") && problems[0].detail.contains("c.yaml:4:1"), "{}", problems[0].detail);
    assert!(!problems[0].detail.contains("front matter"), "{}", problems[0].detail);
}

#[test]
fn the_parsing_settings_combine_key_by_key() {
    let mut settings = Settings::new();
    settings.set_parsing("dollar_math", Value::Bool(false), Package("a.yaml", 1));
    settings.set_parsing("comments", Value::Bool(false), Package("a.yaml", 2));
    settings.set_parsing("comments", Value::Bool(true), Document(1));
    let parsing = settings.parsing().unwrap();
    assert!(!parsing.dollar_math && parsing.comments);
}

/// A package of one feature, `feature`, whose settings are declared from data with the
/// one key `key`, merged entry by entry over the default entries `stdcore` gives it: read
/// through it, the combined value of the key is kept as written.
fn mirror(feature: &'static str, key: &'static str) -> PackageFactory {
    struct Mirror(&'static str, &'static str, Map<String, Value>);
    impl FlmFeature for Mirror {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new(self.0, "Mirror")
        }
        fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            Ok(())
        }
        fn settings(&self) -> Option<SettingsDecl> {
            let schema = serde_json::json!({ "type": "object", "properties": { self.1: {} } });
            let decl = SettingsDecl::new(self.0).config_data(schema, "Mirror.").unwrap();
            Some(decl.merge_entries_data(self.1, self.2.clone()).unwrap())
        }
    }
    let defaults = factory().unwrap().settings(feature).unwrap().default_entries(key).unwrap().clone();
    let feature = Arc::new(Mirror(feature, key, defaults));
    PackageFactory::new("mirror", move |_| Ok(PackageEntry::new("mirror").feature(feature.clone()))).unwrap()
}

/// The value `settings` give the key `key` of the feature `feature` of `stdcore`, which is
/// merged entry by entry, combined as [`Cascade::read`] combines it; or the problems that
/// reading the settings through `stdcore` finds.
fn entries_of(
    settings: &Settings,
    feature: &'static str,
    key: &'static str,
) -> Result<Vec<(String, Value)>, Vec<Problem<TestOrigin>>> {
    settings.read(&[&factory().unwrap()]).map_err(|failure| failure.errors)?;
    let mirror = mirror(feature, key);
    let read = settings.read(&[&mirror]).map_err(|failure| failure.errors)?;
    let config = read.packages[mirror.name()].config::<DataValue>(feature).unwrap();
    Ok(config.0[key].as_object().unwrap().clone().into_iter().collect())
}

/// For each key merged entry by entry: its feature, its name, and an entry it accepts.
fn merged_keys() -> Vec<(&'static str, &'static str, Value)> {
    let words = serde_json::json!({ "base": { "one": "aside", "other": "asides" } });
    vec![
        ("userdefs", "macros", serde_json::json!({ "content": "x" })),
        ("userdefs", "environments", serde_json::json!({ "content": "x" })),
        ("userdefs", "specials", serde_json::json!({ "content": "x" })),
        ("xnotes", "kinds", serde_json::json!({ "words": words.clone() })),
        ("theorems", "classes", serde_json::json!({ "numbered": true, "shared_counter": false, "heading": "bold", "closing_mark": null, "relation_ref": false })),
        ("theorems", "kinds", serde_json::json!({ "class": "theoremlike", "words": words, "expects_proof": false })),
        ("floats", "types", serde_json::json!({ "handlers": ["includegraphics"] })),
        ("quote", "environments", serde_json::json!({ "sections": ["lines"], "bare": "lines" })),
        ("lists", "environments", serde_json::json!({ "numbered": false, "labels": ["-"], "ref_labels": ["-"], "nesting": "bare" })),
        ("headings", "headings", serde_json::json!({ "level": 1 })),
        ("headings", "commands", serde_json::json!({ "heading": "section" })),
        ("annotations", "annotators", serde_json::json!({})),
        ("numbering", "styles", serde_json::json!({ "ranges": false })),
        ("graphics", "strategy", serde_json::json!("copy")),
        ("graphics", "convert", serde_json::json!([{ "from": "pdf", "to": "png" }])),
        ("graphics", "magnification", serde_json::json!({ "raster": 1.5 })),
    ]
}

#[test]
fn a_key_merged_entry_by_entry_is_combined_one_entry_at_a_time() {
    let factory = factory().unwrap();
    let marked: usize = factory
        .features()
        .iter()
        .filter_map(|feature| feature.settings.as_ref())
        .map(|decl| decl.marks().iter().filter(|mark| mark.merge_entries).count())
        .sum();
    assert_eq!(marked, merged_keys().len(), "every key merged entry by entry is tested");
    for (feature, key, entry) in merged_keys() {
        let defaults: Vec<String> =
            factory.settings(feature).unwrap().default_entries(key).unwrap().keys().cloned().collect();
        let map = |pairs: &[(&str, Value)]| {
            Value::Object(pairs.iter().map(|(name, value)| (name.to_string(), value.clone())).collect())
        };
        let mut settings = Settings::new();
        // The front matter, written first, still stands after the packages it outranks.
        let replaced = defaults.first().cloned().unwrap_or_else(|| "new-a".into());
        let removed = defaults.get(1).cloned().unwrap_or_else(|| "new-b".into());
        settings.set(feature, key, map(&[(&replaced, entry.clone()), (&removed, Value::Bool(false))]), Document(3));
        settings.set(feature, key, map(&[("new-a", entry.clone())]), Package("a.yaml", 2));
        settings.set(feature, key, map(&[("new-b", entry.clone())]), Package("b.yaml", 2));
        let combined = entries_of(&settings, feature, key).unwrap_or_else(|problems| panic!("{feature}.{key}: {problems:?}"));
        let names: Vec<&str> = combined.iter().map(|(name, _)| name.as_str()).collect();
        let mut expected: Vec<String> = defaults.clone();
        expected.extend(["new-a".to_string(), "new-b".to_string()]);
        expected.retain(|name| *name != removed);
        assert_eq!(names, expected, "{feature}.{key}: the defaults first, then the packages' in order");
        // The entry the front matter replaced is its data, whole.
        let value = &combined.iter().find(|(name, _)| *name == replaced).unwrap().1;
        assert_eq!(value, &entry, "{feature}.{key}");
        // Two peers giving one entry two values: one problem, for that entry.
        let mut peers = Settings::new();
        peers.set(feature, key, map(&[("new-a", entry.clone())]), Package("a.yaml", 2));
        peers.set(feature, key, map(&[("new-a", Value::Null)]), Package("c.yaml", 4));
        let problems = entries_of(&peers, feature, key).unwrap_err();
        assert_eq!(problems.len(), 1, "{feature}.{key}: {problems:?}");
        assert!(problems[0].detail.contains("the entry `new-a`"), "{}", problems[0].detail);
    }
}

#[test]
fn a_key_not_merged_entry_by_entry_is_replaced_whole_a_list_included() {
    let factory = factory().unwrap();
    let mut settings = Settings::new();
    settings.set("html", "heading_tags", serde_json::json!({"section": "h2"}), Package("a.yaml", 2));
    settings.set("html", "heading_tags", serde_json::json!({"subsection": "h3"}), Document(3));
    settings.set("pieces", "include", serde_json::json!(["flm.a"]), Package("a.yaml", 3));
    settings.set("pieces", "include", serde_json::json!(["flm.b"]), Document(4));
    let read = settings.read(&[&factory]).unwrap();
    let html = read.render.get::<HtmlRenderOptions>().unwrap();
    let tags: Vec<(&str, &str)> = html.heading_tags.iter().map(|(name, tag)| (name.as_str(), tag.as_str())).collect();
    assert_eq!(tags, [("subsection", "h3")]);
    let pieces = read.render.get::<PiecesRenderOptions>().unwrap();
    let include: Vec<&str> = pieces.include.iter().map(Name::as_str).collect();
    assert_eq!(include, ["flm.b"]);
}

#[test]
fn a_marked_key_whose_value_is_no_map_is_a_problem() {
    let mut settings = Settings::new();
    settings.set("floats", "types", serde_json::json!(["figure"]), Document(3));
    let factory = factory().unwrap();
    let problems = settings.read(&[&factory]).unwrap_err().errors;
    assert!(problems[0].detail.contains("must be a map"), "{problems:?}");
    assert_eq!(problems[0].origin, Some(Document(3)));
}

/// The relation of packages each reaching the ones listed: `("a.yaml", &["b.yaml"])`.
fn reach(pairs: &[(&str, &[&str])]) -> Reach {
    pairs.iter().map(|(from, to)| (Name::from(*from), to.iter().map(|to| Name::from(*to)).collect())).collect()
}

/// The width in force when the packages write it in the order `writes`, each a file, a
/// width and a line; or the problems.
fn width_of(
    reach: &Reach,
    writes: &[(&'static str, u32, u32)],
) -> Result<SettingValue<TestOrigin>, Vec<Problem<TestOrigin>>> {
    let mut settings = Settings::with_reach(reach.clone());
    for (file, width, line) in writes {
        settings.set("text", "width", Value::from(*width), Package(file, *line));
    }
    settings.read(&[&factory().unwrap()]).map_err(|failure| failure.errors)?;
    Ok(settings.value("text", "width").unwrap())
}

#[test]
fn a_package_wins_over_the_packages_it_reaches_whatever_the_order_written() {
    // `a` uses `b`, which uses `c`; `d` uses none of them.
    let reach = reach(&[("a.yaml", &["b.yaml", "c.yaml"]), ("b.yaml", &["c.yaml"]), ("d.yaml", &[])]);
    let orders: [[(&str, u32, u32); 3]; 3] = [
        [("a.yaml", 30, 2), ("b.yaml", 40, 3), ("c.yaml", 50, 4)],
        [("c.yaml", 50, 4), ("b.yaml", 40, 3), ("a.yaml", 30, 2)],
        [("b.yaml", 40, 3), ("a.yaml", 30, 2), ("c.yaml", 50, 4)],
    ];
    for order in orders {
        let value = width_of(&reach, &order).unwrap();
        assert_eq!((value.value, value.origin), (Value::from(30), Package("a.yaml", 2)));
    }
    let value = width_of(&reach, &[("c.yaml", 50, 4), ("b.yaml", 40, 3)]).unwrap();
    assert_eq!((value.value, value.origin), (Value::from(40), Package("b.yaml", 3)));
    // `b`, then its peer `d`, then `a` which uses `b`: `a` wins over `b` and is a peer of
    // `d`, with which it agrees or disagrees.
    let value = width_of(&reach, &[("b.yaml", 40, 3), ("d.yaml", 30, 5), ("a.yaml", 30, 2)]).unwrap();
    assert_eq!(value.value, Value::from(30));
    let problems = width_of(&reach, &[("b.yaml", 40, 3), ("d.yaml", 60, 5), ("a.yaml", 30, 2)]).unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    let detail = &problems[0].detail;
    assert!(detail.contains("d.yaml:5:1") && detail.contains("a.yaml:2:1") && !detail.contains("b.yaml"), "{detail}");
}

#[test]
fn packages_that_reach_each_other_are_peers() {
    let direct = reach(&[("a.yaml", &["a.yaml", "b.yaml"]), ("b.yaml", &["a.yaml", "b.yaml"])]);
    let through = reach(&[
        ("a.yaml", &["a.yaml", "b.yaml", "c.yaml"]),
        ("b.yaml", &["a.yaml", "b.yaml", "c.yaml"]),
        ("c.yaml", &["a.yaml", "b.yaml", "c.yaml"]),
    ]);
    for reach in [direct, through] {
        let problems = width_of(&reach, &[("a.yaml", 30, 2), ("b.yaml", 40, 3)]).unwrap_err();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(width_of(&reach, &[("a.yaml", 30, 2), ("b.yaml", 30, 3)]).is_ok());
    }
}

#[test]
fn a_package_wins_over_the_packages_it_reaches_for_every_kind_of_setting() {
    let factory = factory().unwrap();
    // `a` uses `b`; `a`'s values are written first, so that the order written is not
    // what decides.
    let reach = reach(&[("a.yaml", &["b.yaml"])]);
    // One entry of a key merged entry by entry: `a` replaces `footnote`; `b`'s `aside`
    // stays.
    let words = serde_json::json!({ "base": { "one": "x", "other": "xs" } });
    let entry = |command: &str| serde_json::json!({ "commands": [command], "words": words });
    let mut settings = Settings::with_reach(reach.clone());
    settings.set("xnotes", "kinds", serde_json::json!({ "footnote": entry("fn") }), Package("a.yaml", 2));
    settings.set("xnotes", "kinds", serde_json::json!({ "footnote": entry("note"), "aside": entry("aside") }), Package("b.yaml", 3));
    let combined = entries_of(&settings, "xnotes", "kinds").unwrap();
    let commands: Vec<(&str, &Value)> = combined.iter().map(|(name, value)| (name.as_str(), &value["commands"][0])).collect();
    assert_eq!(commands, [("footnote", &Value::from("fn")), ("aside", &Value::from("aside"))]);
    // A feature's switch, both ways.
    let switch = |a_off: bool| {
        let mut settings = Settings::with_reach(reach.clone());
        let record = |settings: &mut Settings, off: bool, origin: TestOrigin| {
            if off { settings.switch_off("text", origin) } else { settings.switch_on("text", origin) }
        };
        record(&mut settings, a_off, Package("a.yaml", 2));
        record(&mut settings, !a_off, Package("b.yaml", 2));
        assert!(settings.read(&[&factory]).is_ok());
        settings.switched_off("text").is_some()
    };
    assert!(switch(true) && !switch(false));
    // A workflow's render settings, and one field of a workflow.
    let mut settings = Settings::with_reach(reach.clone());
    settings.set("text", "width", Value::from(30), Workflow("text", "a.yaml", 2));
    settings.set("text", "width", Value::from(40), Workflow("text", "b.yaml", 2));
    let outputs = |names: &[&str], origin: TestOrigin| vec![(String::from("outputs"), serde_json::json!(names), origin)];
    settings.add_workflow_fields("text", outputs(&["text"], Package("a.yaml", 4)));
    settings.add_workflow_fields("text", outputs(&["html"], Package("b.yaml", 5)));
    let read = settings.read(&[&factory]).unwrap();
    assert_eq!(read.render.get::<TextRenderOptions>().unwrap().width, Some(30));
    let field = settings.workflow_field("text", "outputs").unwrap();
    assert_eq!((field.value, field.origin), (serde_json::json!(["text"]), Package("a.yaml", 4)));
    // What the workflows run with: the field in force, and a workflow written with
    // settings alone, which has no field.
    settings.add_workflow_fields("quiet", []);
    let workflows = settings.workflows();
    let fields = |name: &str| -> Vec<(String, Value)> {
        workflows[name].iter().map(|(field, written)| (field.clone(), written.value.clone())).collect()
    };
    assert_eq!(fields("text"), [(String::from("outputs"), serde_json::json!(["text"]))]);
    assert_eq!(fields("quiet"), []);
    // The parsing settings.
    let mut settings = Settings::with_reach(reach);
    settings.set_parsing("comments", Value::Bool(false), Package("a.yaml", 1));
    settings.set_parsing("comments", Value::Bool(true), Package("b.yaml", 1));
    assert!(!settings.parsing().unwrap().comments);
}

#[test]
fn an_origin_without_a_peer_takes_the_value_written_last() {
    // A reach among packages changes nothing in a rank without peers.
    let mut settings = Settings::with_reach(reach(&[("a.yaml", &["b.yaml"])]));
    settings.set("text", "width", Value::from(30), Document(2));
    settings.set("text", "width", Value::from(40), Document(5));
    assert_eq!(settings.value("text", "width").unwrap().origin, Document(5));
    settings.set("text", "width", Value::from(50), CommandLine(2));
    settings.set("text", "width", Value::from(60), CommandLine(1));
    let value = settings.value("text", "width").unwrap();
    assert_eq!((value.value, value.origin), (Value::from(60), CommandLine(1)));
    // Values that differ within such a rank are no problem.
    assert!(settings.read(&[&factory().unwrap()]).is_ok());
    settings.switch_off("text", Document(6));
    settings.switch_on("text", Document(7));
    assert!(settings.switched_off("text").is_none());
}

#[test]
fn a_higher_source_is_suggested_only_where_a_document_can_write_the_setting() {
    let factory = factory().unwrap();
    let suggests = |problems: Vec<Problem<TestOrigin>>| -> bool {
        assert_eq!(problems.len(), 1, "{problems:?}");
        problems[0].detail.contains("or set the value in the document's front matter")
    };
    // Packages that disagree on a key: the front matter can decide it.
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(30), Package("a.yaml", 2));
    settings.set("text", "width", Value::from(40), Package("b.yaml", 3));
    assert!(suggests(settings.read(&[&factory]).unwrap_err().errors));
    // On a field of a workflow, which no document writes.
    let mut settings = Settings::new();
    settings.add_workflow_fields("text", [(String::from("docs"), Value::from("A"), Package("a.yaml", 2))]);
    settings.add_workflow_fields("text", [(String::from("docs"), Value::from("B"), Package("b.yaml", 3))]);
    assert!(!suggests(settings.read(&[&factory]).unwrap_err().errors));
    // Between the settings packages give a workflow, an origin with no higher source.
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(30), Workflow("w", "a.yaml", 2));
    settings.set("text", "width", Value::from(40), Workflow("w", "b.yaml", 3));
    let problems = settings.read(&[&factory]).unwrap_err().errors;
    assert!(!suggests(problems.clone()));
    assert!(problems[0].detail.starts_with("the packages that set `text.width` must agree"), "{}", problems[0].detail);
}

/// A feature `files` whose settings are declared from data: `bib` and `list` paths, and
/// `entries` merged entry by entry, each entry's `file` a path.
struct Files;

impl FlmFeature for Files {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("files", "Files")
    }

    fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        Ok(())
    }

    fn settings(&self) -> Option<SettingsDecl> {
        let schema = serde_json::json!({ "type": "object", "properties": { "bib": {}, "list": {}, "entries": {} } });
        let decl = SettingsDecl::new("files")
            .config_data(schema, "Files.")
            .and_then(|decl| decl.merge_entries_data("entries", Map::new()))
            .unwrap();
        Some(decl.path("bib").path("list").path("entries.*.file"))
    }
}

/// A feature `nosettings` that takes no settings.
struct NoSettings;

impl FlmFeature for NoSettings {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("nosettings", "No settings")
    }

    fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        Ok(())
    }
}

#[test]
fn a_path_is_resolved_by_the_origin_s_own_rule() {
    let factory = PackageFactory::new("files", |_| Ok(PackageEntry::new("files").feature(Arc::new(Files)))).unwrap();
    let mut settings = Settings::new();
    settings.set("files", "bib", Value::from("refs.bib"), Package("pkg/a.yaml", 2));
    settings.set("files", "list", serde_json::json!(["x.bib", "/abs/y.bib"]), Package("flows/w.yaml", 3));
    settings.set("files", "entries", serde_json::json!({ "one": { "file": "one.txt" } }), Package("p/a.yaml", 4));
    settings.set("files", "entries", serde_json::json!({ "two": { "file": "two.txt" } }), Package("q/b.yaml", 5));
    settings.set("files", "entries", serde_json::json!({ "three": { "file": "three.txt" } }), CommandLine(1));
    let read = settings.read(&[&factory]).unwrap();
    let config = read.packages[factory.name()].config::<DataValue>("files").unwrap();
    assert_eq!(config.0["bib"], "pkg/refs.bib");
    assert_eq!(config.0["list"], serde_json::json!(["flows/x.bib", "/abs/y.bib"]));
    // Each entry by its own origin; the command line's is left as written.
    let files: Vec<&Value> = ["one", "two", "three"].iter().map(|name| &config.0["entries"][name]["file"]).collect();
    assert_eq!(files, ["p/one.txt", "q/two.txt", "three.txt"]);
}

#[test]
fn the_settings_in_effect_hold_a_merged_key_s_combined_value_with_each_entry_s_origin() {
    let factory = factory().unwrap();
    let defaults: Vec<String> =
        factory.settings("xnotes").unwrap().default_entries("kinds").unwrap().keys().cloned().collect();
    let removed = defaults.first().cloned().expect("a default entry");
    let entry = serde_json::json!({ "words": { "base": { "one": "aside", "other": "asides" } } });
    let mut settings = Settings::new();
    settings.set("xnotes", "kinds", serde_json::json!({ "aside": entry.clone() }), Package("a.yaml", 2));
    settings.set("xnotes", "kinds", serde_json::json!({ removed.clone(): false }), Document(3));
    let in_effect = settings.in_effect(&[&factory]);
    let kinds = &in_effect["xnotes"].keys["kinds"];
    // The feature's own entries first, less the one the front matter removed, then the
    // package's; the value is the one `read` reads.
    let names: Vec<&str> = kinds.value.as_object().unwrap().keys().map(String::as_str).collect();
    let mut expected: Vec<&str> = defaults.iter().map(String::as_str).filter(|name| *name != removed).collect();
    expected.push("aside");
    assert_eq!(names, expected);
    assert_eq!(kinds.value["aside"], entry);
    let read: Map<String, Value> = entries_of(&settings, "xnotes", "kinds").unwrap().into_iter().collect();
    assert_eq!(kinds.value, Value::Object(read));
    // Each entry a source wrote has its origin, the removed one included; the key's origin
    // is the highest source's.
    let origins: Vec<(&str, &TestOrigin)> =
        kinds.entry_origins.iter().map(|(name, origin)| (name.as_str(), origin)).collect();
    let mut expected = vec![("aside", &Package("a.yaml", 2)), (removed.as_str(), &Document(3))];
    expected.sort_by_key(|(name, _)| *name);
    assert_eq!(origins, expected);
    assert_eq!(kinds.origin, Document(3));
}

#[test]
fn the_settings_in_effect_hold_each_key_s_resulting_value_and_origin_paths_resolved() {
    let factory = factory().unwrap();
    let mut settings = Settings::new();
    settings.set("text", "width", Value::from(60), Defaults);
    settings.set("text", "width", Value::from(70), Package("a.yaml", 2));
    settings.set("text", "width", Value::from(80), Document(3));
    settings.set("nosuch", "x", Value::from(1), Document(4));
    settings.set("text", "nosuchkey", Value::from(1), Document(5));
    let in_effect = settings.in_effect(&[&factory]);
    let width = &in_effect["text"].keys["width"];
    assert_eq!((&width.value, &width.origin), (&Value::from(80), &Document(3)));
    assert!(width.entry_origins.is_empty());
    assert_eq!(in_effect["text"].switched_off, None);
    // A key the feature does not declare is listed like the others.
    let undeclared = &in_effect["text"].keys["nosuchkey"];
    assert_eq!((&undeclared.value, &undeclared.origin), (&Value::from(1), &Document(5)));
    // A feature that no factory holds is left out.
    assert!(!in_effect.contains_key("nosuch"));
    // A path is resolved by its origin, each entry of a merged key by its own.
    let files = PackageFactory::new("files", |_| {
        Ok(PackageEntry::new("files").feature(Arc::new(Files)).feature(Arc::new(NoSettings)))
    })
    .unwrap();
    let mut settings = Settings::new();
    settings.set("files", "bib", Value::from("refs.bib"), Package("pkg/a.yaml", 2));
    settings.set("files", "entries", serde_json::json!({ "one": { "file": "one.txt" } }), Package("p/a.yaml", 4));
    settings.set("files", "entries", serde_json::json!({ "two": { "file": "two.txt" } }), CommandLine(1));
    settings.set("nosettings", "x", Value::from(1), Document(5));
    let in_effect = settings.in_effect(&[&files]);
    // The keys of a feature that takes no settings are left out.
    assert!(in_effect["nosettings"].keys.is_empty());
    let keys = &in_effect["files"].keys;
    assert_eq!((&keys["bib"].value, &keys["bib"].origin), (&Value::from("pkg/refs.bib"), &Package("pkg/a.yaml", 2)));
    assert_eq!(keys["entries"].value, serde_json::json!({ "one": { "file": "p/one.txt" }, "two": { "file": "two.txt" } }));
    assert_eq!(keys["entries"].entry_origins["one"], Package("p/a.yaml", 4));
    assert_eq!(keys["entries"].entry_origins["two"], CommandLine(1));
}

#[test]
fn the_settings_in_effect_say_where_a_feature_was_switched_off() {
    let factory = factory().unwrap();
    let mut settings = Settings::new();
    settings.switch_off("text", Document(1));
    settings.set("text", "width", Value::from(30), CommandLine(1));
    let in_effect = settings.in_effect(&[&factory]);
    assert_eq!(in_effect["text"].switched_off, Some(Document(1)));
    assert!(in_effect["text"].keys.is_empty(), "no key of a feature switched off is in effect");
    // A higher source switches it on again, and its keys are in effect.
    settings.switch_on("text", CommandLine(2));
    let in_effect = settings.in_effect(&[&factory]);
    assert_eq!(in_effect["text"].switched_off, None);
    assert_eq!(in_effect["text"].keys["width"].value, Value::from(30));
}
