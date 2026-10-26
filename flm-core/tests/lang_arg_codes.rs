//! The argument codes (`flm_core::lang::mint::codes`): what each code of the
//! alphabet reads, the three shapes of an argument list, a user definition
//! declared in settings with each code, the codes that are none, the mode and
//! scoped packages of an argument written as a map, the code an argument
//! keeps, and a user definition declared in settings with a structured syntax
//! that no code writes.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::lang::mint::{
    arg_def_from_code, ArgCodeProblem, ArgData, ArgDataFields, ArgDef, ArgListData, ArgStdKind,
};
use flm_core::model::names::Name;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{self, StdcoreConfig};
use serde_json::{json, Value};
use techy::latexlike::Mode;

use common::html::fragment;
use common::{callables, lang, macro_with, parse};

// --- test 1: what each code reads ---------------------------------------------------------------

/// What a macro `\m` declared with `code` alone reads in `source`: whether the
/// argument is there, and the source text of its content.
fn read(code: &str, source: &str) -> (bool, Option<String>) {
    let arg = arg_def_from_code(code).unwrap_or_else(|error| panic!("{code}: {error}"));
    let lang = lang(vec![macro_with("t", "m", vec![arg])]);
    let fragment = parse(&lang, source);
    assert!(fragment.diags().is_empty(), "{code} on {source}: {:?}", fragment.diags());
    let node = callables(&fragment).into_iter().next().expect("the macro");
    let provided = node.arguments().unwrap().get(0).unwrap().is_provided();
    let content = node.argument_content_nodes(0).and_then(|nodes| nodes.source_text().map(str::to_string));
    (provided, content)
}

#[test]
fn each_code_reads_the_toolkit_s_example() {
    let cases = [
        ("{", r"\m{arg}", "arg"),
        ("m", r"\m{arg}", "arg"),
        ("m", r"\m1", "1"),
        ("[", r"\m[arg]", "arg"),
        ("o", r"\m[arg]", "arg"),
        ("*", r"\m*", "*"),
        ("s", r"\m*", "*"),
        ("t!", r"\m!", "!"),
        ("t*", r"\m*", "*"),
        ("r()", r"\m(arg)", "arg"),
        ("d<>", r"\m<arg>", "arg"),
        ("v", r"\m+raw %text+", "raw %text"),
        ("v+-", r"\m+raw %text-", "raw %text"),
        ("e{^_}", r"\m^{a}_{b}", "^{a}_{b}"),
        ("e{^_}", r"\m_{b}^{a}", "_{b}^{a}"),
        ("AnyDelimited", r"\m<arg>", "arg"),
        ("AnyDelimited", r"\m{arg}", "arg"),
        ("AnyDelimitedOptional", r"\m(arg)", "arg"),
        ("BracedOnly", r"\m{arg}", "arg"),
    ];
    for (code, source, content) in cases {
        assert_eq!(read(code, source), (true, Some(content.to_string())), "{code} on {source}");
    }
}

#[test]
fn each_optional_code_reads_no_argument_when_nothing_is_written() {
    for code in ["[", "o", "*", "s", "t!", "d<>", "e{^_}", "AnyDelimitedOptional"] {
        let (provided, _) = read(code, r"\m x");
        assert!(!provided, "{code}");
    }
}

#[test]
fn a_mandatory_code_with_no_fallback_reports_the_argument_missing() {
    for code in ["r()", "AnyDelimited", "BracedOnly"] {
        let lang = lang(vec![macro_with("t", "m", vec![arg_def_from_code(code).unwrap()])]);
        let fragment = parse(&lang, r"\m x");
        assert!(!fragment.diags().is_empty(), "{code}");
    }
}

// --- test 2: the three shapes of an argument list -----------------------------------------------

/// The identities of the arguments an argument list written as `data`
/// declares, and their names.
fn declared(data: Value) -> Vec<(Option<String>, Vec<u8>)> {
    let list: ArgListData = serde_json::from_value(data).unwrap();
    list.arg_defs().unwrap().into_iter().map(|arg: ArgDef| (arg.name, arg.kind.identity())).collect()
}

#[test]
fn the_three_shapes_of_a_list_declare_the_same_arguments() {
    let text = declared(json!("[{*"));
    assert_eq!(text.len(), 3);
    assert_eq!(declared(json!(["[", "{", "*"])), text);
    assert_eq!(declared(json!([{ "code": "[" }, { "code": "{" }, "*"])), text);
    assert_eq!(declared(json!(["o", "m", "s"])), text);
    // A map names its argument; the other entries stay unnamed.
    let named = declared(json!([{ "code": "[", "name": "size" }, "{"]));
    assert_eq!(named[0].0.as_deref(), Some("size"));
    assert_eq!(named[1].0, None);
    assert!(declared(json!("")).is_empty());
}

#[test]
fn a_user_definition_reads_its_list_in_each_shape() {
    let shapes = [json!("[{"), json!(["[", "{"]), json!([{ "code": "[" }, { "code": "{" }])];
    let configs: Vec<String> = shapes
        .into_iter()
        .map(|args| format!("{:?}", userdefs(json!({ "macros": { "hi": { "content": "#1#2", "args": args } } }))))
        .collect();
    assert_eq!(configs[0], configs[1]);
    assert_eq!(configs[0], configs[2]);
}

// --- test 3: a user definition declared with each code the library did not read before ----------

/// The `userdefs` configuration read from `data`, as a front matter's
/// settings are read.
fn userdefs(data: Value) -> UserdefsConfig {
    let factory = stdcore::factory().unwrap();
    let decl = factory.settings("userdefs").unwrap();
    let settings = decl.read(data.as_object().unwrap()).unwrap_or_else(|errors| panic!("{errors:?}"));
    settings.config::<UserdefsConfig>().cloned().unwrap()
}

/// The registry over the standard package whose `userdefs` settings are `data`.
fn registry(data: Value) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().userdefs(userdefs(data)))
}

#[test]
fn a_user_definition_declared_with_each_new_code_expands_its_argument() {
    // The code, a use, and what the template `[#1]` gives in running text and
    // in a formula. Each pair of an embellishments argument is a group whose
    // opening delimiter is the marker. Running text writes a group's content
    // and not its delimiters, for this argument as for `r()`, `d<>` and `[`:
    // `\m^{a}_{b}` gives `[ab]`. In a formula the argument is written as its
    // source, markers included: `[^{a}_{b}]`.
    let cases = [
        ("t!", r"\m!", "[!]", "[!]"),
        ("r()", r"\m(arg)", "[arg]", "[arg]"),
        ("d<>", r"\m<arg>", "[arg]", "[arg]"),
        ("v", r"\m|raw|", "[raw]", "[raw]"),
        ("v||", r"\m|raw|", "[raw]", "[raw]"),
        ("e{^_}", r"\m^{a}_{b}", "[ab]", "[^{a}_{b}]"),
        ("AnyDelimited", r"\m(arg)", "[arg]", "[arg]"),
        ("AnyDelimitedOptional", r"\m<arg>", "[arg]", "[arg]"),
        ("BracedOnly", r"\m{arg}", "[arg]", "[arg]"),
    ];
    for (code, source, in_text, in_math) in cases {
        let registry = registry(json!({ "macros": { "m": { "content": "[#1]", "args": [code] } } }));
        let document = format!("A {source} B \\({source}\\)");
        let text = common::spine::text_at(&registry, &document, None);
        assert_eq!(text, format!("A {in_text} B \\({in_math}\\)\n"), "{code}");
        let run = common::stdcore::run_workflow(&registry, html::WORKFLOW, &[&document], &Overrides::none());
        assert!(run.diagnostics().iter().next().is_none(), "{code}: {}", run.diagnostics().render_all());
        let page = fragment(&run);
        assert!(page.starts_with(&format!("A {in_text} B <span")), "{code}: {page}");
        assert!(page.contains(&format!("\\({in_math}\\)</span>")), "{code}: {page}");
    }
}

// --- test 4: a code that is none, and a code lacking characters ---------------------------------

#[test]
fn a_code_outside_the_alphabet_and_an_incomplete_code_are_met_when_the_settings_are_read() {
    let factory = stdcore::factory().unwrap();
    let decl = factory.settings("userdefs").unwrap();
    for (args, problem) in [
        (json!(["q"]), ArgCodeProblem::Unknown),
        (json!([{ "code": "Q" }]), ArgCodeProblem::Unknown),
        (json!("{q"), ArgCodeProblem::Unknown),
        (json!(["r("]), ArgCodeProblem::Incomplete),
        (json!(["t"]), ArgCodeProblem::Incomplete),
        (json!(["e{}"]), ArgCodeProblem::Incomplete),
        (json!("r()"), ArgCodeProblem::Incomplete),
    ] {
        let data = json!({ "macros": { "hi": { "content": "", "args": args.clone() } } });
        let errors = decl.read(data.as_object().unwrap()).unwrap_err();
        assert_eq!(errors.len(), 1, "{args}");
        assert_eq!(errors[0].path, "userdefs.macros.hi", "{args}");
        let code = match &args {
            Value::String(text) => text.chars().find(|c| arg_def_from_code(&c.to_string()).is_err()).unwrap().to_string(),
            Value::Array(items) => match &items[0] {
                Value::String(code) => code.clone(),
                other => other["code"].as_str().unwrap().to_string(),
            },
            _ => unreachable!(),
        };
        let expected = arg_def_from_code(&code).unwrap_err();
        assert_eq!(expected.problem, problem, "{args}");
        let message = errors[0].to_string();
        assert!(message.contains(&expected.to_string()), "{args}: {message}");
    }
}

// --- test 5: the mode and the scoped packages of an argument written as a map ---------------------

/// The argument declaration of one argument written as `data`.
fn arg_from(data: Value) -> ArgDef {
    let data: ArgData = serde_json::from_value(data).unwrap_or_else(|error| panic!("{error}"));
    ArgDef::try_from(data).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn a_map_reads_the_argument_s_mode_and_scoped_packages() {
    let math = arg_from(json!({ "code": "{", "mode": "math" }));
    assert_eq!(math.mode, Some(Mode::Math));
    assert_eq!(arg_from(json!({ "code": "{", "mode": "text" })).mode, Some(Mode::Text));
    let scoped = arg_from(json!({ "code": "[", "scoped": ["mypack.notes", "mypack.marks"] }));
    assert_eq!(scoped.scoped, vec![Name::from("mypack.notes"), Name::from("mypack.marks")]);
    // Neither key written: the mode the command is written in, no scoped package.
    let plain = arg_from(json!({ "code": "{" }));
    assert_eq!(plain.mode, None);
    assert!(plain.scoped.is_empty());
    // An argument list in the list shape reads the keys the same way.
    let list: ArgListData = serde_json::from_value(json!([{ "code": "{", "mode": "math", "scoped": ["p"] }, "["])).unwrap();
    let args = list.arg_defs().unwrap();
    assert_eq!((args[0].mode, args[0].scoped.clone()), (Some(Mode::Math), vec![Name::from("p")]));
    assert_eq!((args[1].mode, args[1].scoped.len()), (None, 0));
}

#[test]
fn a_map_with_an_unknown_key_or_mode_is_refused() {
    assert!(serde_json::from_value::<ArgDataFields>(json!({ "code": "{", "colour": "red" })).is_err());
    assert!(serde_json::from_value::<ArgData>(json!({ "code": "{", "colour": "red" })).is_err());
    assert!(serde_json::from_value::<ArgData>(json!({ "code": "{", "mode": "display" })).is_err());
}

// --- test 6: an argument keeps the code it was read from -----------------------------------------

#[test]
fn an_argument_read_from_a_code_keeps_the_code() {
    assert_eq!(arg_def_from_code("[").unwrap().code(), Some("["));
    assert_eq!(arg_def_from_code("r()").unwrap().code(), Some("r()"));
    // `{` and `m` choose the same parser; each declaration keeps its own code.
    assert_eq!(arg_def_from_code("m").unwrap().code(), Some("m"));
    assert_eq!(arg_from(json!({ "code": "e{^_}", "name": "marks" })).code(), Some("e{^_}"));
    let list: ArgListData = serde_json::from_value(json!("[{")).unwrap();
    let codes: Vec<Option<String>> = list.arg_defs().unwrap().iter().map(|arg| arg.code().map(str::to_string)).collect();
    assert_eq!(codes, vec![Some("[".to_string()), Some("{".to_string())]);
    // The builders keep the code.
    assert_eq!(arg_def_from_code("{").unwrap().blocks().docs("text").code(), Some("{"));
}

#[test]
fn an_argument_built_from_a_parser_has_no_code() {
    assert_eq!(ArgDef::new(ArgStdKind::Optional).code(), None);
    assert_eq!(ArgDef::mandatory("text").code(), None);
    assert_eq!(ArgDef::optional("size").code(), None);
}

// --- test 7: a user definition declared with a structured syntax that no code writes --------------

/// The text that the template `[#1]` gives for `source` in running text, with
/// a macro `\m` declared in settings with the one argument `arg`.
fn expanded(arg: Value, source: &str) -> String {
    let registry = registry(json!({ "macros": { "m": { "content": "[#1]", "args": [arg] } } }));
    let run = common::stdcore::run_workflow(&registry, html::WORKFLOW, &[source], &Overrides::none());
    assert!(run.diagnostics().iter().next().is_none(), "{source}: {}", run.diagnostics().render_all());
    common::spine::text_at(&registry, source, None)
}

#[test]
fn a_characters_only_group_declared_in_settings_reads_an_escape_as_a_character() {
    assert_eq!(expanded(json!({ "syntax": "chars-group" }), r"A \m{a\b} B"), "A [a\\b] B\n");
}

#[test]
fn a_terminated_argument_declared_in_settings_reads_its_opener() {
    let arg = json!({ "syntax": "terminated", "terminator": "\\endm", "opener": ":", "head": "" });
    assert_eq!(expanded(arg, r"A \m: text \endm B"), "A [ text ] B\n");
}
