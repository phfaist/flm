//! The exports of the generated prelude (step E2b, the plan's section
//! 11.3): every API module imported from a test package, its exports listed
//! and compared with the inventory golden (each export is an item of the
//! module's Rust source, or a submodule of the surface exported as a
//! namespace) and with the exports of the hand-written stubs the
//! generated ones replaced (the same names, but for the differences the
//! plan's section 5.2 allows: the empty class `Lower` is gone, and the new
//! module `flm:techy/error` exports `Severity`).

#![cfg(feature = "quickjs")]

use std::collections::BTreeMap;

use flm_ext::quickjs::{eval_for_test, JsHost, JsSettings, TestPackage};
use serde_json::Value;

/// The inventory golden, the surface as the generator read it.
const INVENTORY: &str = include_str!("goldens/api_surface/inventory.json");

/// The exports of the hand-written stubs before E2b, by module (`flm:` left
/// out), with the allowed differences applied.
const EXPECTED: &[(&str, &[&str])] = &[
    ("core/artifact", &["media_type_of", "Artifact", "ArtifactId"]),
    ("core/data/store", &["StageOut", "TablePiece"]),
    ("core/data/table", &["ExtTableRead"]),
    (
        "core/lowering",
        &[
            "Staged", "chars_at_lowering", "spelling_at_lowering", "LoweringDecl", "DefaultMark", "Lowering", "LoweringRule", "LowerCx",
            "Lowerer", "pres_tree",
        ],
    ),
    (
        "core/registry",
        &["FeatureBuilder", "CallableKind", "DefContent", "StyleDef", "ScriptDef", "StandardTable", "BackendDef", "LibraryStage", "OutputDef"],
    ),
    ("core/run", &["StageCx", "RewriteCx"]),
    ("core/services", &["Arity"]),
    ("core/stage", &["StageDecl", "Needs", "Effect", "Scope", "Severity", "TreesTable", "RewriteDecl"]),
    ("host", &["HostService", "FilesScope", "Files"]),
    // WP-4a: the submodules `mint` and `text` as namespaces (and, since the
    // merge with E6, `structural`).
    ("lang", &["BuiltLang", "FlmDiagnostics", "mint", "structural", "text"]),
    (
        "lang/mint",
        &["ArgLayout", "BodyLayout", "Mode", "CallableKind", "FallbackKind", "ArgDef", "arg_def_from_code", "BodyDef", "SyntaxDef", "ElementDef", "FallbackDef", "ScopedPackageDef"],
    ),
    ("lang/rewrites/blocks", &["argument_holds_blocks"]),
    (
        "lang/rewrites/userdefs",
        &[
            "DefinitionKind", "DefinitionModes", "ExpandPolicy", "Definition", "Expansion", "UserDefinition", "ModeVariant", "CompiledTemplate",
            "PlaceholderBindings", "DefinitionBindings", "Staged", "ExpandCx", "Expander",
            // WP-3: the templates a handler compiles.
            "TemplateLanguage", "ValueBindings", "compile_template_with", "instantiate",
            // WP-4b: the class of a template specification.
            "TemplateSpec",
        ],
    ),
    (
        "lang/structural",
        &[
            "TEXT", "COMMENT", "CONTAINER", "GROUP", "MATH", "VERBATIM", "PARAGRAPH", "PARBREAK", "PLACEHOLDER", "UNKNOWN_MACRO",
            "UNKNOWN_ENVIRONMENT", "NAMES",
        ],
    ),
    ("lang/text", &["chars_text", "read_chars"]),
    ("model/aspects", &["InlineSpacing", "Placement", "BlockRole", "RefPoint", "NumberSource", "AnchorPolicy", "ContentRef", "AcceptedRefTypes"]),
    ("model/element", &["Aspect", "ElementSpec"]),
    ("stdcore/backends/html", &["TAG"]),
    ("stdcore/backends/lowering", &["LoweredTrees", "LoweringStage", "lowered_table_name"]),
    (
        "stdcore/backends/latex",
        &[
            "ALL", "ARG_SLOT", "BEGIN_SLOT", "DEFINITION", "END_SLOT", "ENVIRONMENT", "LatexArgShape", "MACRO", "RAW", "RECOMPOSED_BACKEND",
            // WP-4a: the payloads' classes and the two slots' declarations.
            "LatexDefinitionKind", "LatexMacroPayload", "LatexEnvironmentPayload", "LatexRawPayload", "LatexDefinitionPayload",
            "LatexPreambleDef", "LatexPackageDef", "packages", "preamble",
        ],
    ),
    (
        "stdcore/backends/latex/packages",
        &[
            "ADJUSTBOX", "AMSMATH", "AMSTHM", "CAPTION", "ENUMITEM", "FVEXTRA", "GRAPHICX", "HYPERREF", "ID_PREFIX", "NEWUNICODECHAR", "PHFCC",
            "PHFTHM", "SLOT", "TABULARRAY", "TEXTCASE", "XCOLOR", "ZREF_CLEVER", "package_id", "LatexPackageDef",
        ],
    ),
    (
        "stdcore/backends/latex/preamble",
        &[
            "HEADING", "HYPERREF", "ID_PREFIX", "IF_BLANK", "LABELS", "LABEL_DISPLAY_TEXT", "MATH_ENVIRONMENT", "REFS", "SLOT", "TAB_WIDTH",
            "USER_ARGS", "VERBATIM", "ZREF_CLEVER_TYPE", "piece_id", "LatexPreambleDef",
        ],
    ),
    (
        "stdcore/backends/pres",
        &[
            "PAR", "BLOCK", "BLOCK_HEADING", "RUNIN_HEADING", "SPAN", "LIST", "CELLS", "LINES", "LINE", "VERBATIM", "MATH_PASSTHROUGH", "MATH_TEXT",
            "LINK", "IMAGE", "UNSUPPORTED", "DROPPED", "UNKNOWN", "DECO_RUNIN", "DECO_FINAL", "DECO_MARGIN", "DECO_MARK", "SUBTREE", "ALL",
            "BODY_SLOT", "LABEL_SLOT", "ITEM_SLOT", "CELL_SLOT",
        ],
    ),
    ("stdcore/features/citations", &["PROVIDERS_SLOT", "CitationEntry", "ProviderCx"]),
    ("stdcore/features/compose", &["TEMPLATES_SLOT", "ENGINES_SLOT", "TemplateCx"]),
    (
        "stdcore/features/math",
        &[
            "MATH", "ENVIRONMENT", "ENVIRONMENTS", "CONTENT", "RENDERED", "KIND", "COUNTER", "ANCHORS", "COUNTERS", "LINES_PACKAGE",
            "LINEBREAK_COMMAND", "TAG_COMMAND", "NONUMBER_COMMAND", "TEXT", "TEXT_COMMANDS", "ENSUREMATH", "equation_lines", "formula_display",
            "environment_numbered", "line_frames", "number_text", "stage_tag_content", "environment_opening", "line_closing",
            "environment_closing", "group_opening", "group_closing",
            // The line writer of the recomposed LaTeX and the newline rule.
            "stage_lines", "normalize_line_newlines",
        ],
    ),
    (
        "stdcore/features/numbering",
        &[
            "COUNTERS", "COUNTERS_SLOT", "STYLES_SLOT", "WITHIN_JOINERS_SLOT", "NUMBER_WRITERS_SLOT", "PARENT_SETS_SLOT", "Counters",
            // WP-4a: the declarations' classes.
            "CounterDecl", "CounterStyleDecl", "ParentSetDecl", "Within", "PluralWords", "PrefixWords", "Delimiters", "NumberTemplate",
            "FormatOptions",
        ],
    ),
    (
        "stdcore/features/refs",
        &[
            "Anchors", "ANCHORS", "SAFE_LABELS", "LABELS", "REF_RESOLUTIONS", "FIELDS_ARG", "LABEL_FIELD", "TARGET_ARG", "LABEL_PACKAGE",
            "TARGET_ARG_ROLE", "DEFINED_LABEL_ARG_ROLE", "label_fields_arg",
            "label_uses", "point_labels", "stage_label", "stage_label_group",
        ],
    ),
    ("stdcore/features/userdefs", &["element_name_under", "expand_in_tree_rule_for", "write_math_run", "LoweringRule"]),
    ("techy/error", &["Severity"]),
    ("techy/extract", &["parse_keyval_drop_annotations", "KeyVals", "KeyValEntry"]),
    ("techy/node", &["Node", "NodeKindName"]),
    ("techy/transform", &["Staged", "Restage", "Restager", "RestagedArgument", "RestagedSlot"]),
];

/// The exports of every module of the inventory, as a test package's script
/// lists them, by module path.
fn exports(modules: &[String]) -> BTreeMap<String, Vec<String>> {
    let host = JsHost::new(JsSettings::default());
    let mut main = String::new();
    for (position, module) in modules.iter().enumerate() {
        main.push_str(&format!("import * as M{position} from \"flm:{module}\";\n"));
    }
    main.push_str("export function exports() {\n  return JSON.stringify({\n");
    for (position, module) in modules.iter().enumerate() {
        main.push_str(&format!("    \"{module}\": Object.keys(M{position}),\n"));
    }
    main.push_str("  });\n}\n");
    let package = TestPackage::new(&host, &[("main.js", main.as_str())]).expect("the test package");
    let answer = eval_for_test(&host, &package, "return main.exports();").expect("the script runs");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    let answer: Value = match answer {
        Value::String(text) => serde_json::from_str(&text).expect("JSON"),
        other => other,
    };
    serde_json::from_value(answer).expect("a map of lists of names")
}

#[test]
fn every_api_module_exports_the_names_of_its_rust_items_and_of_the_stubs_before() {
    let inventory: Value = serde_json::from_str(INVENTORY).expect("the inventory golden is JSON");
    let inventory = inventory.as_array().expect("a list of modules");
    let modules: Vec<String> = inventory.iter().map(|module| module["module"].as_str().expect("a module path").to_string()).collect();
    let listed = exports(&modules);
    for module in inventory {
        let path = module["module"].as_str().expect("a module path");
        let items: Vec<&str> = module["items"].as_array().expect("items").iter().filter_map(|item| item["name"].as_str()).collect();
        for name in &listed[path] {
            let namespace = modules.contains(&format!("{path}/{name}"));
            assert!(
                items.contains(&name.as_str()) || namespace,
                "the export `{name}` of `flm:{path}` must be an item of the inventory's module or one of its submodules"
            );
        }
    }
    let expected: BTreeMap<String, Vec<String>> = EXPECTED
        .iter()
        .map(|(path, names)| {
            let mut names: Vec<String> = names.iter().map(ToString::to_string).collect();
            names.sort();
            (path.to_string(), names)
        })
        .collect();
    let mut listed = listed;
    listed.values_mut().for_each(|names| names.sort());
    assert_eq!(listed, expected);
}
