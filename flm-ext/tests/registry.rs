//! The crate's manifest and its reach into the parser toolkit, and the test
//! helpers' registry.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{html_of, latex_of, registry_with, text_of};

// --- the manifest --------------------------------------------------------------------------

/// The names of the dependencies a manifest declares, in every table of
/// dependencies.
fn dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line.ends_with("dependencies]");
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            names.push(name.trim().to_string());
        }
    }
    names
}

/// The members of the workspace, by the folder names its manifest lists.
fn workspace_members(manifest: &str) -> Vec<String> {
    let line = manifest.lines().find(|line| line.trim_start().starts_with("members")).unwrap_or("");
    line.split('"').skip(1).step_by(2).map(String::from).collect()
}

#[test]
fn the_manifest_names_no_crate_of_the_workspace_but_the_library_and_not_the_toolkit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("the manifest");
    let workspace = fs::read_to_string(root.join("../Cargo.toml")).expect("the workspace");
    let members = workspace_members(&workspace);
    assert!(members.iter().any(|m| m == "flm-core"), "{members:?}");
    assert!(members.iter().any(|m| m == "flm-ext"), "{members:?}");
    let names = dependencies(&manifest);
    assert!(names.iter().any(|n| n == "flm-core"), "{names:?}");
    for name in &names {
        let other_member = members.iter().any(|m| m == name) && name != "flm-core";
        assert!(!other_member, "the crate depends on the workspace's `{name}`");
        assert!(!name.starts_with("techy"), "the crate names the toolkit `{name}`");
    }
}

// --- the toolkit ---------------------------------------------------------------------------

/// Every `.rs` file under `dir`, at any depth.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Whether `line` reaches the toolkit other than through the library's
/// re-export: a `use` of the toolkit's crate, an `extern crate` of it, or a
/// path that begins with the toolkit's name (a path segment of that name
/// further along a path — `flm_core::techy`, or the extension API's own
/// module `api::techy` — is not the crate).
fn reaches_the_toolkit(line: &str, toolkit: &str) -> bool {
    let trimmed = line.trim_start();
    let declared = [format!("use {toolkit}"), format!("pub use {toolkit}"), format!("extern crate {toolkit}")];
    if declared.iter().any(|start| trimmed.starts_with(start.as_str())) {
        return true;
    }
    let path = format!("{toolkit}::");
    line.match_indices(path.as_str()).any(|(at, _)| {
        line[..at].chars().last().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == ':'))
    })
}

#[test]
fn no_source_file_reaches_techy_but_through_flm_core() {
    // The toolkit's name is assembled, so that this file does not name it in
    // the forms it looks for.
    let toolkit = ["te", "chy"].concat();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("tests"), &mut files);
    assert!(files.len() > 10, "{files:?}");
    let mut found = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("a source file");
        for (number, line) in text.lines().enumerate() {
            if reaches_the_toolkit(line, &toolkit) {
                found.push(format!("{}:{}: {line}", file.display(), number + 1));
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
    assert!(reaches_the_toolkit(&format!("use {toolkit}::core;"), &toolkit));
    assert!(reaches_the_toolkit(&format!("let x = {toolkit}::Mode::Text;"), &toolkit));
    assert!(!reaches_the_toolkit(&format!("use flm_core::{toolkit}::core;"), &toolkit));
    assert!(!reaches_the_toolkit(&format!("use crate::api::{toolkit}::node::Node;"), &toolkit));
}

// --- the helpers ---------------------------------------------------------------------------

#[test]
fn a_registry_with_the_standard_package_alone_renders_through_each_output() {
    let registry = registry_with([]);
    let (text, diagnostics) = text_of(&registry, "Some \\emph{text}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Some text.\n");
    let (html, diagnostics) = html_of(&registry, "Some \\emph{text}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(html.contains("<em"), "{html}");
    let (latex, diagnostics) = latex_of(&registry, "Some \\emph{text}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(latex.contains("\\emph{text}"), "{latex}");
}

// --- the builder's rows of E4: collection_member, scoped_package, late_init, finalizer --------

mod builder_rows {
    use std::sync::Arc;

    use flm_core::core::plan::{Plan, PlanRequest};
    use flm_core::stdcore::backends::text;
    use flm_ext::adapter::handles::Entry;
    use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
    use flm_ext::api::core::registry::{FeatureBuilder, Finalizer, LateInit};
    use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
    use flm_ext::api::lang::mint::{ArgDef, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef};
    use flm_ext::api::lang::{BuiltLang, FlmDiagnostics};
    use flm_ext::api::model::element::ElementSpec;
    use flm_ext::api::techy::error::Diagnostic;
    use flm_ext::api::techy::node::Node;
    use flm_ext::ExtError;
    use serde_json::{json, Value};

    use crate::common::double::{stage, Double};
    use crate::common::json_double::JsonDouble;
    use crate::common::{diagnostics, registry_with, run, text_of, try_registry_with};

    /// The feature `count` of the package `ex`: a capability `ex.counted`, a
    /// check stage providing it that reports `ex.ran`, and what `extra`
    /// declares.
    fn counting(extra: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static) -> Double {
        Double::new("ex").feature("count", move |b| {
            b.capability("ex.counted", "The count ran.", None)?;
            let decl = StageDecl::new("ex.count", Effect::Check, Scope::Document).provides("ex.counted");
            b.stage(stage(decl, Needs::new(), |_, out| out.diags(Diagnostic::note("ex.ran", "the count ran"))))?;
            extra(b)
        })
    }

    /// Whether the count ran in the plain text workflow over a short text.
    fn ran(registry: &Arc<flm_core::core::registry::FlmRegistry>) -> bool {
        diagnostics(&run(registry, text::WORKFLOW, "Some words.")).iter().any(|d| d.starts_with("ex.ran"))
    }

    #[test]
    fn a_scripted_member_joins_a_collection_of_a_rust_feature() {
        let joined = registry_with([counting(|b| b.collection_member("flm.validated", "ex.counted")).package()]);
        assert!(ran(&joined));
        let alone = registry_with([counting(|_| Ok(())).package()]);
        assert!(!ran(&alone));
    }

    #[test]
    fn a_member_outside_the_stem_is_refused() {
        let error = try_registry_with([counting(|b| b.collection_member("flm.validated", "other.counted")).package()])
            .expect_err("a member outside the stem")
            .to_string();
        assert!(error.contains("flm.ext.name-outside-extension"), "{error}");
        assert!(error.contains("other.counted"), "{error}");
    }

    #[test]
    fn a_member_of_a_collection_no_feature_declares_is_dropped() {
        let registry = registry_with([counting(|b| b.collection_member("ex.nowhere", "ex.counted")).package()]);
        assert!(registry.build_diagnostics().is_empty());
        assert!(!ran(&registry));
    }

    // --- scoped_package --------------------------------------------------------------------

    /// A body emitting the lowered body of an environment, or the lowered
    /// first argument of a macro.
    struct Emit;

    impl Lower for Emit {
        fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
            if node.element_name() == "ex.box" {
                Ok(Lowering::Emit(lw.lower_body(cx, node)?))
            } else {
                Ok(Lowering::Emit(lw.lower_arg(cx, node, 0)?))
            }
        }
    }

    /// The rule of `element`.
    fn emit_rule(element: &str) -> LoweringRule {
        LoweringRule::new(format!("{element}.lower.default"), element, LoweringDecl::new(), Arc::new(Emit), DefaultMark::Everywhere)
    }

    /// The environment `exbox`, whose body names the scoped package `package`.
    fn exbox(feature: &str, package: &str) -> ElementDef {
        let body = BodyDef::new().scoped(package);
        ElementDef::new(feature, ["exbox"], SyntaxDef::Environment { args: Vec::new(), body }, ElementSpec::new("ex.box"))
    }

    /// The package `ex`: `exbox` and the scoped package `ex.hints` of `\exhint{…}`.
    fn hints(package_named: &'static str) -> Double {
        Double::new("ex").feature("boxes", move |b| {
            b.element(exbox(&b.feature(), package_named))?;
            let hint = ElementDef::new(
                b.feature(),
                ["exhint"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                ElementSpec::new("ex.hint"),
            );
            b.scoped_package(ScopedPackageDef::new("ex.hints", vec![hint], "The hints of a box."))?;
            b.lowering_rule(emit_rule("ex.box"))?;
            b.lowering_rule(emit_rule("ex.hint"))
        })
    }

    #[test]
    fn a_scoped_command_is_legal_inside_its_body_alone() {
        let registry = registry_with([hints("ex.hints").package()]);
        let (inside, diagnostics) = text_of(&registry, "\\begin{exbox}Look: \\exhint{here}.\\end{exbox}");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(inside.contains("Look: here."), "{inside}");
        let (_, diagnostics) = text_of(&registry, "Outside \\exhint{there}.");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].contains("exhint"), "{diagnostics:?}");
    }

    #[test]
    fn a_body_naming_a_scoped_package_no_feature_declares_fails_the_build() {
        let error = try_registry_with([hints("ex.nowhere").package()]).expect_err("an unknown scoped package").to_string();
        assert!(error.contains("ex.nowhere"), "{error}");
    }

    #[test]
    fn a_scoped_package_named_outside_the_stem_is_refused() {
        let package = Double::new("ex")
            .feature("boxes", |b| b.scoped_package(ScopedPackageDef::new("other.hints", Vec::new(), "")))
            .package();
        let error = try_registry_with([package]).expect_err("outside the stem").to_string();
        assert!(error.contains("flm.ext.name-outside-extension") && error.contains("other.hints"), "{error}");
    }

    #[test]
    fn a_scoped_package_and_a_scoped_body_through_the_dispatcher() {
        let double = JsonDouble::new("ex").feature("boxes", |js, b| {
            let body = js.lend_lower(|js, node, cx, lw| {
                let method = if js.call(node, "element_name", json!([]))? == json!("ex.box") { "lower_body" } else { "lower_arg" };
                let args = if method == "lower_body" { json!([cx, node]) } else { json!([cx, node, 0]) };
                Ok(json!({ "emit": js.call(lw, method, args)? }))
            });
            js.call(
                b,
                "element",
                json!([{
                    "names": ["exbox"],
                    "syntax": { "environment": { "args": [], "body": { "scoped": ["ex.hints"] } } },
                    "element": { "name": "ex.box" },
                }]),
            )?;
            let hint = json!({ "names": ["exhint"], "syntax": { "macro": { "args": "{" } }, "element": { "name": "ex.hint" } });
            js.call(b, "scoped_package", json!([{ "name": "ex.hints", "elements": [hint], "docs": "Hints." }]))?;
            for element in ["ex.box", "ex.hint"] {
                let rule = js.call(
                    &crate::common::json_double::module("core/lowering"),
                    "LoweringRule::new",
                    json!([format!("{element}.lower.default"), element, {}, body, "everywhere"]),
                )?;
                js.call(b, "lowering_rule", json!([rule]))?;
            }
            Ok(())
        });
        let registry = registry_with([double.package()]);
        let (inside, diagnostics) = text_of(&registry, "\\begin{exbox}Look: \\exhint{here}.\\end{exbox}");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(inside.contains("Look: here."), "{inside}");
        let (_, diagnostics) = text_of(&registry, "Outside \\exhint{there}.");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }

    // --- late_init -------------------------------------------------------------------------

    /// A late-initialization hook that pushes `diagnostic`, or fails.
    struct Late(Option<Diagnostic>);

    impl LateInit for Late {
        fn late_init<'c>(&self, _: &'c dyn BuiltLang, diags: &'c mut dyn FlmDiagnostics) -> Result<(), ExtError> {
            match &self.0 {
                Some(diagnostic) => diags.push(diagnostic.clone()),
                None => Err(ExtError::Handler { detail: "the hook gave up".to_string(), data: Value::Null }),
            }
        }
    }

    #[test]
    fn a_late_init_hook_s_diagnostic_is_among_the_build_diagnostics() {
        let warning = Diagnostic::warning("ex.late", "a problem with the configuration");
        let package = Double::new("ex").feature("late", move |b| b.late_init(Arc::new(Late(Some(warning.clone()))))).package();
        let registry = registry_with([package]);
        let found: Vec<String> =
            registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
        assert_eq!(found, ["ex.late: a problem with the configuration"]);
    }

    #[test]
    fn a_late_init_hook_s_diagnostic_outside_the_stem_and_a_failing_hook_fail_the_build() {
        let outside = Diagnostic::warning("other.late", "x");
        let package = Double::new("ex").feature("late", move |b| b.late_init(Arc::new(Late(Some(outside.clone()))))).package();
        let error = try_registry_with([package]).expect_err("outside the stem").to_string();
        assert!(error.contains("flm.ext.handler-failed: ") && error.contains("other.late"), "{error}");
        let failing = Double::new("ex").feature("late", |b| b.late_init(Arc::new(Late(None)))).package();
        let error = try_registry_with([failing]).expect_err("a failing hook").to_string();
        assert!(error.contains("flm.ext.handler-failed: the hook gave up"), "{error}");
    }

    #[test]
    fn a_late_init_hook_through_the_dispatcher_pushes_a_diagnostic() {
        /// A hook acting through the dispatcher on the handles it is lent.
        struct JsonLate;
        impl LateInit for JsonLate {
            fn late_init<'c>(&self, lang: &'c dyn BuiltLang, diags: &'c mut dyn FlmDiagnostics) -> Result<(), ExtError> {
                use flm_ext::adapter::{dispatch, handles::CallTable, values::handle_from_value, values::handle_to_value};
                let mut table = CallTable::new(7000);
                let _lang = handle_to_value(table.lend(Entry::BuiltLang(flm_ext::adapter::handles::LangEntry::Lent(lang))));
                let diags = handle_to_value(table.lend(Entry::FlmDiagnostics(diags)));
                let diagnostic = json!({ "id": "ex.late", "severity": "warning", "message": "from data" });
                dispatch::call(&mut table, handle_from_value(&diags)?, "push", &json!([diagnostic]))?;
                Ok(())
            }
        }
        let double = JsonDouble::new("ex").feature("late", |js, b| {
            let hook = js.lend(Entry::LateInit(Arc::new(JsonLate)));
            js.call(b, "late_init", json!([hook]))?;
            Ok(())
        });
        let registry = registry_with([double.package()]);
        let found: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
        assert_eq!(found, ["from data"]);
    }

    // --- finalizer -------------------------------------------------------------------------

    /// A finalizer whose work is `f`.
    struct Finish(Box<FinishFn>);

    /// A finalizer's work.
    type FinishFn = dyn Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync;

    impl Finalizer for Finish {
        fn finalize(&self, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
            (self.0)(b)
        }
    }

    #[test]
    fn a_finalizer_declares_a_table_and_its_stage_in_the_second_round() {
        let package = Double::new("ex")
            .feature("late", |b| {
                b.finalizer(Arc::new(Finish(Box::new(|b| {
                    b.table("ex.late-table", "A table of the second round.")?;
                    let decl = StageDecl::new("ex.fill", Effect::Compute, Scope::Document).writes("ex.late-table");
                    b.stage(stage(decl, Needs::new(), |_, out| out.table("ex.late-table")?.push(json!(1))))?;
                    b.peer_table("flm.labels", "Read later.")
                }))))
            })
            .package();
        let registry = registry_with([package]);
        assert!(Plan::build(&registry, &PlanRequest::new().demand("ex.late-table")).is_ok());
    }

    #[test]
    fn a_finalizer_cannot_register_another_and_a_failing_one_fails_the_build() {
        let nested = Double::new("ex")
            .feature("late", |b| b.finalizer(Arc::new(Finish(Box::new(|b| b.finalizer(Arc::new(Finish(Box::new(|_| Ok(()))))))))))
            .package();
        let error = try_registry_with([nested]).expect_err("a finalizer of the second round").to_string();
        // `flm-core`'s `ContributionAfterAssembly`, through the handler's failure.
        assert!(error.contains("flm.ext.handler-failed: ") && error.contains("register a finalizer"), "{error}");
        let failing = Double::new("ex")
            .feature("late", |b| {
                b.finalizer(Arc::new(Finish(Box::new(|_| Err(ExtError::Handler { detail: "no".to_string(), data: Value::Null })))))
            })
            .package();
        let error = try_registry_with([failing]).expect_err("a failing finalizer").to_string();
        assert!(error.contains("flm.ext.handler-failed: no"), "{error}");
    }
}
