//! The exports of the generated prelude: every API module imported from a
//! test package, its exports listed and compared with the inventory golden
//! (each export is an item of the module's Rust source, a submodule of the
//! surface exported as a namespace, or the class of a sequence of a list)
//! and with the exports listed below,
//! module by module.

#![cfg(feature = "quickjs")]

use std::collections::BTreeMap;

use flm_ext::quickjs::{eval_for_test, JsHost, JsSettings, TestPackage};
use serde_json::Value;

/// The inventory golden, the surface as the generator read it.
const INVENTORY: &str = include_str!("goldens/api_surface/inventory.json");

/// The classes of the sequences a binding makes of a list of the extension
/// API, which no Rust item stands for, by module: the sequences of a list of
/// nodes and of a list of staged nodes.
const SEQUENCES: &[(&str, &str)] = &[("techy/node", "Nodes"), ("lang/rewrites/userdefs", "StagedNodes")];

/// The exports of each module (`flm:` left out), listed by hand.
const EXPECTED: &[(&str, &[&str])] = &[
    ("core/artifact", &["media_type_of", "Artifact", "ArtifactId"]),
    ("core/data/store", &["StageOut", "TablePiece"]),
    ("core/data/table", &["ExtTableRead", "Rows"]),
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
    ("core/run", &["StageCx", "Walk", "Trees", "RewriteCx"]),
    ("core/services", &["Arity"]),
    // `ReadError` is a type alone: `flm.d.ts` declares it, and the module exports nothing.
    ("core/settings", &[]),
    ("core/stage", &["StageDecl", "Needs", "Effect", "Scope", "Severity", "TreesTable", "RewriteDecl"]),
    ("host", &["HostService", "Files", "path"]),
    // The submodules `copy`, `mint`, `structural` and `text`, as namespaces.
    ("lang", &["BuiltLang", "FlmDiagnostics", "copy", "mint", "structural", "text"]),
    ("lang/copy", &[]),
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
            // The templates a handler compiles.
            "TemplateLanguage", "ValueBindings", "compile_template_with", "instantiate",
            // The class of a template specification.
            "TemplateSpec",
            // The values of the extra placeholders a hook computes.
            "ExtraValues",
            // The sequence of a list of staged nodes.
            "StagedNodes",
        ],
    ),
    (
        "lang/structural",
        &[
            "TEXT", "COMMENT", "CONTAINER", "GROUP", "MATH", "VERBATIM", "OPAQUE_WRAPPER", "PARAGRAPH", "PARBREAK", "PLACEHOLDER",
            "UNKNOWN_MACRO", "UNKNOWN_ENVIRONMENT", "NAMES",
            // The name of the language's field element.
            "FIELD",
        ],
    ),
    ("lang/text", &["chars_text", "read_chars"]),
    ("math_passthrough", &["math_rule_named", "environment_rule_named", "DefaultMark", "LoweringRule", "latex"]),
    ("math_passthrough/latex", &["math_rule_named", "environment_rule_named", "LoweringRule"]),
    ("model/aspects", &["InlineSpacing", "Placement", "BlockRole", "RefPoint", "NumberSource", "AnchorPolicy", "ContentRef", "AcceptedRefTypes"]),
    ("model/element", &["Aspect", "ElementSpec"]),
    ("progress", &["Unit", "ProgressScope", "Pending", "Task", "Counted"]),
    ("stdlib/backends/html", &["TAG"]),
    ("stdlib/backends/lowering", &["LoweredTrees", "LoweringStage", "lowered_table_name", "is_blank", "stage_trimmed"]),
    (
        "stdlib/backends/latex",
        &[
            "ALL", "ARG_SLOT", "BEGIN_SLOT", "DEFINITION", "END_SLOT", "ENVIRONMENT", "LatexArgShape", "MACRO", "RAW", "RECOMPOSED_BACKEND",
            // The payloads' classes and the two slots' declarations.
            "LatexDefinitionKind", "LatexMacroPayload", "LatexEnvironmentPayload", "LatexRawPayload", "LatexDefinitionPayload",
            "LatexPreambleDef", "LatexPackageDef", "packages", "preamble",
        ],
    ),
    (
        "stdlib/backends/latex/packages",
        &[
            "ADJUSTBOX", "AMSMATH", "AMSTHM", "CAPTION", "ENUMITEM", "FVEXTRA", "GRAPHICX", "HYPERREF", "ID_PREFIX", "NEWUNICODECHAR", "PHFCC",
            "PHFTHM", "SLOT", "TABULARRAY", "TEXTCASE", "XCOLOR", "ZREF_CLEVER", "package_id", "LatexPackageDef",
        ],
    ),
    (
        "stdlib/backends/latex/preamble",
        &[
            "HEADING", "HYPERREF", "ID_PREFIX", "IF_BLANK", "LABELS", "LABEL_DISPLAY_TEXT", "MATH_ENVIRONMENT", "REFS", "SLOT", "TAB_WIDTH",
            "USER_ARGS", "VERBATIM", "ZREF_CLEVER_TYPE", "piece_id", "LatexPreambleDef",
        ],
    ),
    (
        "stdlib/backends/pres",
        &[
            "PAR", "BLOCK", "BLOCK_HEADING", "RUNIN_HEADING", "SPAN", "LIST", "CELLS", "LINES", "LINE", "VERBATIM", "MATH_PASSTHROUGH", "MATH_TEXT",
            "LINK", "IMAGE", "UNSUPPORTED", "DROPPED", "UNKNOWN", "DECO_RUNIN", "DECO_FINAL", "DECO_MARGIN", "DECO_MARK", "SUBTREE", "ALL",
            "BODY_SLOT", "LABEL_SLOT", "ITEM_SLOT", "CELL_SLOT",
        ],
    ),
    ("stdlib/features/citations", &["PROVIDERS_SLOT", "CitationEntry", "ProviderCx"]),
    ("stdlib/features/compose", &["TEMPLATES_SLOT", "ENGINES_SLOT", "TemplateCx"]),
    (
        "stdlib/features/math",
        &[
            "MATH", "ENVIRONMENT", "ENVIRONMENTS", "CONTENT", "RENDERED", "KIND", "COUNTER", "ANCHORS", "COUNTERS", "LINES_PACKAGE",
            "LINEBREAK_COMMAND", "TAG_COMMAND", "NONUMBER_COMMAND", "TEXT", "TEXT_COMMANDS", "ENSUREMATH", "FORMULA_ROLE",
            "formula_attributes", "equation_lines", "formula_display",
            "environment_numbered", "line_frames", "number_text", "stage_tag_content", "environment_opening", "line_closing",
            "environment_closing", "group_opening", "group_closing",
            // The line writer of the recomposed LaTeX and the newline rule.
            "stage_lines", "normalize_line_newlines",
            // The writer of a formula's TeX with holes.
            "group_tex", "line_tex",
        ],
    ),
    (
        "stdlib/features/numbering",
        &[
            "COUNTERS", "COUNTERS_SLOT", "STYLES_SLOT", "WITHIN_JOINERS_SLOT", "NUMBER_WRITERS_SLOT", "PARENT_SETS_SLOT", "Counters",
            // The declarations' classes.
            "CounterDecl", "CounterStyleDecl", "ParentSetDecl", "Within", "PluralWords", "PrefixWords", "Delimiters", "NumberTemplate",
            "FormatOptions",
        ],
    ),
    (
        "stdlib/features/refs",
        &[
            "Anchors", "ANCHORS", "SAFE_LABELS", "LABELS", "REF_RESOLUTIONS", "FIELDS_ARG", "LABEL_FIELD", "TARGET_ARG", "LABEL_PACKAGE",
            "TARGET_ARG_ROLE", "DEFINED_LABEL_ARG_ROLE", "label_fields_arg",
            "label_uses", "point_labels", "stage_label", "stage_label_group",
        ],
    ),
    ("stdlib/features/userdefs", &["element_name_under", "expand_in_tree_rule_for", "write_math_run", "LoweringRule"]),
    ("techy/error", &["Severity"]),
    ("techy/extract", &["parse_keyval_drop_annotations", "KeyVals", "KeyValEntry"]),
    ("techy/node", &["Node", "Descendants", "Tree", "NodeKindName", "Nodes"]),
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
            let sequence = SEQUENCES.contains(&(path, name.as_str()));
            assert!(
                items.contains(&name.as_str()) || namespace || sequence,
                "the export `{name}` of `flm:{path}` must be an item of the inventory's module, one of its submodules or a sequence's class"
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
