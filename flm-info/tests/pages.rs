//! The pages over the standard environment of the library: the registry of `stdcore` and
//! `math-passthrough` with a plan for the workflow `text`, as `tests/describe.rs` builds it.
//!
//! The FLM source of chosen pages is compared with text goldens under
//! `tests/goldens/pages/*.flm`, and the JSON form of a page and of an output with JSON
//! goldens there, written as indented JSON text. Rewrite them with `UPDATE_GOLDEN=1` after
//! reading the diff; they change whenever a documentation text of the standard features
//! changes.
//!
//! The test over every page also writes each page's source under the cargo target's folder
//! for temporary files (`target/tmp/pages-core`, and `target/tmp/pages-host` under the cargo
//! feature `flm-main`), replaced at each run, so that a person can render them with the
//! program: this crate does not depend on the host and cannot render a page itself.

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::dump::golden::assert_text_golden;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_info::Environment;
use flm_info::pages::{
    EnvironmentVariable, ImageFolders, Output, Page, PageKind, ProgramCommand, ProgramExtensions, ProgramFolder,
    ProgramInfo, ProgramOption, ProgramPaths, SourceOptions, TextSource, Tree, source, texts,
};

/// The standard environment, with the plan of the workflow `text` in view.
fn environment() -> Environment {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder.build(&RegistryInputs::new()).expect("the standard registry builds");
    let plan = Plan::for_workflow(&registry, "text", &Overrides::none()).expect("the plan of `text`");
    let factories = vec![
        stdcore::factory().expect("the factory of stdcore"),
        flm_core::math_passthrough::factory().expect("the factory of math-passthrough"),
    ];
    Environment::new(registry, Some(Arc::new(plan)), factories)
}

/// What a program might say about itself, with a character that FLM escapes in each text.
fn program() -> ProgramInfo {
    ProgramInfo {
        version: String::from("0.5.0"),
        commands: vec![ProgramCommand {
            name: String::from("info"),
            help: String::from("Describe the environment {packages, features & more}"),
            options: vec![
                ProgramOption { names: vec![], value_name: Some(String::from("PATH")), help: String::from("A page's path"), default: None },
                ProgramOption {
                    names: vec![String::from("-r"), String::from("--recursive")],
                    value_name: None,
                    help: String::from("Include the pages below in full (100%)"),
                    default: None,
                },
                ProgramOption {
                    names: vec![String::from("-p"), String::from("--profile")],
                    value_name: Some(String::from("PROFILE")),
                    help: String::from("The profile in view"),
                    default: Some(String::from("html")),
                },
            ],
        }],
        environment_variables: vec![EnvironmentVariable {
            name: String::from("FLMPATH"),
            help: String::from("Folders of extensions, separated by `:` (C:\\ on Windows ~ ;)"),
        }],
        paths: ProgramPaths {
            search_path: vec![
                ProgramFolder { path: String::from("ext"), origin: String::from("--extensions-dir"), exists: true },
                ProgramFolder { path: String::from("~/.flm/extensions"), origin: String::from("the user's folder"), exists: false },
            ],
            cache_folder: Some(ProgramFolder { path: String::from("_cache"), origin: String::new(), exists: false }),
            image_folders: Some(ImageFolders {
                folders: vec![String::from(".")],
                rule: String::from("Each folder is relative to the folder of the file that writes the image, or absolute."),
            }),
        },
        extensions: ProgramExtensions {
            compiled: vec![String::from("math-passthrough"), String::from("std")],
            found: vec![String::from("narrow")],
            bundled: vec![String::from("flmdoc"), String::from("profile-pdf-latex")],
        },
    }
}

/// The name of the golden file of the page `path`: its segments joined by `-`.
fn golden_name(path: &str) -> String {
    match path.trim_end_matches('/') {
        "" => String::from("root"),
        trimmed => trimmed.replace('/', "-"),
    }
}

fn golden(name: &str, text: &str) {
    assert_text_golden(format!("{}/tests/goldens/pages/{name}", env!("CARGO_MANIFEST_DIR")), text);
}

/// The paths that the `\docref` commands of `source` link to, read by text: the delimited
/// argument after each `\docref`.
fn docrefs(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("\\docref") {
        rest = &rest[at + "\\docref".len()..];
        let mut chars = rest.char_indices();
        let Some((_, open)) = chars.next() else { break };
        let close = if open == '{' { '}' } else { open };
        let mut depth = 0;
        let mut end = None;
        for (index, c) in chars {
            if open == '{' && c == '{' {
                depth += 1;
            } else if c == close {
                if depth == 0 {
                    end = Some(index);
                    break;
                }
                depth -= 1;
            }
        }
        let end = end.expect("a `\\docref` path is closed");
        out.push(rest[open.len_utf8()..end].to_string());
        rest = &rest[end..];
    }
    out
}

#[test]
fn chosen_pages_are_goldens() {
    let tree = Tree::new(&environment(), None);
    for path in [
        "/",
        "features/headings",
        "features/headings/macros/section",
        "features/refs/macros/ref",
        "features/href/macros/url",
        "features/lists/scopedpackages/flm.lists.items",
        "features/lists/scopedpackages/flm.lists.items/macros/item",
        "settings/html",
        "workflows/text",
        "namespaces/",
        "namespaces/stage/flm.refs.resolve",
    ] {
        let page = tree.page(path).unwrap_or_else(|| panic!("the page `{path}`"));
        assert_eq!(page.path, path);
        golden(&format!("{}.flm", golden_name(path)), &source(&page, &SourceOptions::default()));
    }
}

#[test]
fn a_page_as_json_is_a_golden_with_and_without_its_children_in_full() {
    let tree = Tree::new(&environment(), None);
    let page = tree.page("features/headings").expect("the page of the headings feature");
    let shallow = page.to_json(false);
    assert_eq!(shallow["path"], "features/headings");
    assert_eq!(shallow["kind"], "feature");
    assert!(shallow["children"][0]["title"].is_string(), "{shallow}");
    assert!(shallow["children"][0].get("body").is_none());
    golden("page-features-headings.json", &(serde_json::to_string_pretty(&shallow).unwrap() + "\n"));
    let deep = page.to_json(true);
    let child = &deep["children"][0];
    assert_eq!(child["kind"], "command-index");
    assert!(child["children"][0]["body"]["declarations"].is_array(), "{child}");
    golden("page-features-headings-recursive.json", &(serde_json::to_string_pretty(&deep).unwrap() + "\n"));
}

#[test]
fn an_output_holds_its_header_first_then_the_pages() {
    let tree = Tree::new(&environment(), None);
    let pages = ["features/headings/macros/section", "settings/parsing"]
        .iter()
        .map(|path| tree.page(path).expect("the page").to_json(false))
        .collect();
    let output = Output::new(pages);
    let text = serde_json::to_string_pretty(&output).unwrap() + "\n";
    assert!(text.starts_with("{\n  \"meta\": {\n    \"kind\": \"info\""), "{text}");
    assert_eq!(output.meta.format, 1);
    golden("output.json", &text);
}

#[test]
fn a_name_is_looked_up_across_the_pages() {
    let tree = Tree::new(&environment(), None);
    let paths = |name: &str| tree.lookup(name).into_iter().map(|page| page.path).collect::<Vec<_>>();
    assert_eq!(paths("\\section"), ["features/headings/macros/section"]);
    assert_eq!(paths("section"), ["features/headings/macros/section"]);
    assert_eq!(paths("flm.refs.resolve"), ["namespaces/stage/flm.refs.resolve"]);
    // A scoped package's command is found inside its scoped package.
    assert_eq!(paths("\\item"), ["features/lists/scopedpackages/flm.lists.items/macros/item"]);
    // A feature, and a settings key, are found at their own pages.
    assert_eq!(paths("headings"), ["features/headings"]);
    assert_eq!(paths("html.title"), ["settings/html"]);
    assert!(paths("nothing.here").is_empty());
}

#[test]
fn a_path_matches_exactly() {
    let tree = Tree::new(&environment(), None);
    assert!(tree.page("nothing/here").is_none());
    assert!(tree.page("features/nothing").is_none());
    assert!(tree.page("features/headings/macros/nothing").is_none());
    assert!(tree.page("features/headings/macros/").is_none());
    assert!(tree.page("\\section").is_none());
    // The entities of the namespaces whose entities have a page elsewhere have none here.
    assert!(tree.page("namespaces/feature/headings").is_none());
    // The index page of a section may be given without its final slash.
    assert_eq!(tree.page("features").map(|page| page.path), Some(String::from("features/")));
    // Without the program's information, the section `program/` is absent.
    assert!(tree.page("program/").is_none());
    assert!(tree.page("/").unwrap().children.iter().all(|child| child.path != "program/"));
}

/// Every page of the tree: a unique path, a title, children that are pages of the tree
/// whose title and summary the entry repeats, and links to pages of the tree only. The
/// sources are written under the cargo target's folder for temporary files
/// (`target/tmp/<folder>`), for a person to render them.
fn check_every_page(tree: &Tree, folder: &str) -> usize {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(folder);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the folder of the sources");
    let pages: Vec<Page> = tree.walk().collect();
    let mut by_path: BTreeMap<&str, &Page> = BTreeMap::new();
    for page in &pages {
        assert!(by_path.insert(&page.path, page).is_none(), "the path `{}` is walked twice", page.path);
        assert!(!page.title.is_empty(), "the page `{}` has no title", page.path);
        assert_eq!(page.kind == PageKind::Root, page.path == "/", "{}", page.path);
    }
    for (number, page) in pages.iter().enumerate() {
        for child in &page.children {
            let found = by_path.get(child.path.as_str()).unwrap_or_else(|| panic!("the child `{}` of `{}`", child.path, page.path));
            assert_eq!((&found.title, &found.summary), (&child.title, &child.summary), "the entry of `{}`", child.path);
        }
        let text = source(page, &SourceOptions::default());
        for path in docrefs(&text) {
            assert!(by_path.contains_key(path.as_str()), "the page `{}` links to `{path}`, which is no page", page.path);
        }
        std::fs::write(out.join(format!("{number:05}.flm")), &text).expect("the source is written");
    }
    pages.len()
}

#[test]
fn every_page_has_a_unique_path_a_title_and_links_to_pages_only() {
    let count = check_every_page(&Tree::new(&environment(), Some(program())), "pages-core");
    assert!(count > 500, "{count} pages");
}

#[test]
fn the_program_pages_show_what_the_program_says() {
    let tree = Tree::new(&environment(), Some(program()));
    for path in ["program/", "program/paths", "program/extensions"] {
        let page = tree.page(path).unwrap_or_else(|| panic!("the page `{path}`"));
        golden(&format!("{}.flm", golden_name(path)), &source(&page, &SourceOptions::default()));
    }
}

/// The texts a page embeds, each with whose text it is: a command's text and its summary,
/// declared by its feature, of the package that holds it; an index page's own summary, of
/// FLM's own; the summary of each feature on `features/` and on `namespaces/feature`,
/// declared by the feature. Every text listed is in the page's source, as it is: whole, or
/// its summary then the rest.
#[test]
fn a_page_lists_the_texts_it_embeds_with_their_sources() {
    let tree = Tree::new(&environment(), None);
    let feature = |name: &str| TextSource::Feature { feature: name.to_string(), package: Some(String::from("stdcore")) };
    for path in ["features/headings/macros/section", "features/", "namespaces/feature", "/", "settings/text"] {
        let page = tree.page(path).unwrap_or_else(|| panic!("the page `{path}`"));
        let text = source(&page, &SourceOptions::default());
        let embedded = texts(&page);
        assert!(!embedded.is_empty(), "{path}");
        for item in &embedded {
            let summary = flm_info::summary_of(&item.text);
            let rest = item.text.trim()[summary.len()..].trim();
            assert!(text.contains(summary) && text.contains(rest), "the page `{path}` does not embed {:?}", item.text);
        }
    }
    let section = tree.page("features/headings/macros/section").unwrap();
    let embedded = texts(&section);
    assert!(embedded.iter().all(|item| item.source == feature("headings")), "{embedded:?}");
    // The page writes the summary on its own, before the rest of the text.
    assert!(embedded.iter().any(|item| item.text == section.summary), "{embedded:?}");
    for path in ["features/", "namespaces/feature"] {
        let embedded = texts(&tree.page(path).unwrap());
        let refs = tree.page("features/refs").unwrap();
        assert!(embedded.iter().any(|item| item.text == refs.summary && item.source == feature("refs")), "{path}: {embedded:?}");
    }
    let features = tree.page("features/").unwrap();
    assert!(texts(&features).iter().any(|item| item.text == features.summary && item.source == TextSource::Flm));
}

#[test]
fn the_schema_holds_the_page_and_the_output() {
    let schema = flm_info::describe::schema();
    assert!(schema["$defs"]["Page"].is_object());
    assert!(schema["$defs"]["Output"].is_object());
    assert!(schema["$defs"]["Node"].is_object());
}

/// The pages of an environment that the host prepared (cargo feature `flm-main`): a
/// document in memory that uses a data extension found in the folder `_flmextensions`
/// beside it and writes a setting, as `tests/host.rs` prepares it.
#[cfg(feature = "flm-main")]
mod host {
    use std::path::PathBuf;
    use std::sync::Arc;

    use flm_info::Environment;
    use flm_info::pages::{SourceOptions, TextHolder, TextSource, Tree, source, texts};
    use flm_main::files::MemoryFiles;
    use flm_main::packages::ExtensionFolder;
    use flm_main::seed::MathChoice;
    use flm_main::session::{Input, Request, prepare};

    use super::{check_every_page, golden, program};

    const NARROW: &str =
        "extension:\n  description: Narrow text.\n  author: Ada Lovelace\nflm:\n  features:\n    text:\n      width: 20\n";

    /// The document: it uses the extension, writes the width again at line 6, and switches
    /// the feature `toc` off.
    const DOCUMENT: &str = "---\nflm:\n  use: [narrow]\n  features:\n    text:\n      width: 30\n    toc: false\n  layout: \
                            blocks\n---\nHello.\n";

    fn environment() -> Environment {
        let mut files = MemoryFiles::new();
        files.insert("doc/paper.flm", DOCUMENT);
        files.insert("doc/_flmextensions/narrow.yaml", NARROW);
        let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
        request.default_math = MathChoice::processor("passthrough");
        request.profile = Some(String::from("text"));
        request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
        let prepared = prepare(request, Arc::new(files), &[]).expect("the document is prepared");
        Environment::of_prepared(&prepared)
    }

    /// The features of the package `stdmain` that flm-main compiles in only under its cargo
    /// features `graphics` and `citations`, in the order in which the package declares them.
    const OPTIONAL_FEATURES: [&str; 2] = ["graphics-files", "citations-standard"];

    /// The name of the golden file of the root page `/`, which depends on the cargo features
    /// that flm-main is built with.
    ///
    /// The root page lists every feature and every workflow of the registry. flm-main adds
    /// the feature `graphics-files` to its package `stdmain` under its cargo feature
    /// `graphics`, and the feature `citations-standard`, which declares the workflows
    /// `bibtex` and `csl-json`, under its cargo feature `citations`. This crate turns
    /// flm-main on without either cargo feature. Cargo, however, builds a dependency once
    /// per build, with the union of the cargo features that the crates of the build turn on:
    /// in the workspace's build, flm-cli turns both on, so this test runs against a host
    /// that holds both features. Each of the two builds produces a correct root page of its
    /// own, so each has its own golden file.
    ///
    /// The name is `host-root`, followed by `+<feature>` for each of the `OPTIONAL_FEATURES`
    /// that the package `stdmain` declares, then `.flm`. The build is read from the
    /// package's declarations rather than from the registry, so that a registry lacking a
    /// feature that its package declares still fails the comparison. Two golden files exist,
    /// one for each build that the project tests:
    ///
    /// - `host-root.flm`, for the host without the two cargo features: `cargo test -p
    ///   flm-info --features flm-main`;
    /// - `host-root+graphics-files+citations-standard.flm`, for the host as flm-cli builds
    ///   it: `cargo test --workspace`, or `cargo test -p flm-info --features
    ///   flm-main,flm-main/graphics,flm-main/citations`.
    ///
    /// A change to the root page rewrites both files, with `UPDATE_GOLDEN=1` under each
    /// build. A build that turns on only one of the two cargo features has no golden file,
    /// and the test reports the file missing.
    fn root_golden(env: &Environment) -> String {
        let stdmain = env
            .packages()
            .into_iter()
            .find(|package| package.name == "stdmain")
            .expect("the host's environment holds the package `stdmain`");
        let mut name = String::from("host-root");
        for feature in OPTIONAL_FEATURES {
            if stdmain.features.iter().any(|declared| declared == feature) {
                name.push('+');
                name.push_str(feature);
            }
        }
        name + ".flm"
    }

    #[test]
    fn the_host_s_pages_show_the_profile_the_origins_and_the_settings_in_effect() {
        let env = environment();
        let tree = Tree::new(&env, None);
        let root = source(&tree.page("/").expect("the root page"), &SourceOptions::default());
        assert!(root.contains("The profile in view is \\verbcode{text}"), "{root}");
        assert!(root.contains("(from the extension \\verbcode{doc/_flmextensions/narrow.yaml})"), "{root}");
        for (path, name) in [
            ("/", root_golden(&env)),
            ("packages/doc/_flmextensions/narrow.yaml#_", String::from("host-package-narrow.flm")),
            ("settings/text", String::from("host-settings-text.flm")),
            ("settings/front-matter", String::from("host-settings-front-matter.flm")),
        ] {
            let page = tree.page(path).unwrap_or_else(|| panic!("the page `{path}`"));
            golden(&name, &source(&page, &SourceOptions::default()));
        }
        let settings = source(&tree.page("settings/text").unwrap(), &SourceOptions::default());
        assert!(settings.contains("\\docfield{Value in effect}{\\verbcode{30}}"), "{settings}");
        // A feature switched off has no page; its package's page does not link to it.
        assert!(tree.page("features/toc").is_none());
        let stdcore = source(&tree.page("packages/stdcore").unwrap(), &SourceOptions::default());
        assert!(stdcore.contains("\\verbcode{toc} (not in the registry)"), "{stdcore}");
    }

    #[test]
    fn every_page_of_the_host_s_environment_links_to_pages_only() {
        check_every_page(&Tree::new(&environment(), Some(program())), "pages-host");
    }

    /// A data extension that writes a workflow, which no feature declares, and a user
    /// definition, each with a documentation text of two paragraphs (the texts at lines 5
    /// and 11, the entry of the user definition at line 9).
    const DRAFTS: &str = "flm:\n  workflows:\n    draft:\n      extends: text\n      docs: \"A draft.\\n\\nIt extends \
                          `text`.\"\n  features:\n    userdefs:\n      macros:\n        project:\n          content: the \
                          project\n          docs: \"The project.\\n\\nIt is written as text.\"\n";

    /// The environment of a document that uses the extension `drafts`.
    fn drafted() -> Environment {
        using("drafts", DRAFTS)
    }

    /// The environment of a document that uses the data extension `name`, written `data`, in
    /// the folder `_flmextensions` beside it.
    fn using(name: &str, data: &str) -> Environment {
        let mut files = MemoryFiles::new();
        files.insert("doc/draft.flm", format!("---\nflm:\n  use: [{name}]\n---\nHello.\n"));
        files.insert(format!("doc/_flmextensions/{name}.yaml"), data);
        let mut request = Request::new(Input::File(PathBuf::from("doc/draft.flm")));
        request.default_math = MathChoice::processor("passthrough");
        request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
        let prepared = prepare(request, Arc::new(files), &[]).expect("the document is prepared");
        Environment::of_prepared(&prepared)
    }

    /// A text written as data comes from where it was written, both whole and as its
    /// summary, on every page that embeds it: the workflow's on its page, on the list of
    /// workflows, on `/` and on the list of the names of the namespace `workflow`; the user
    /// definition's on its page, on the list of the feature's macros, on the feature's page
    /// (its element value) and on the list of the names of the namespace `element`.
    #[test]
    fn a_text_written_as_data_comes_from_where_it_was_written() {
        let tree = Tree::new(&drafted(), None);
        let package = Some(String::from("doc/_flmextensions/drafts.yaml#_"));
        let source_of = |path: &str, text: &str| -> (TextHolder, Option<String>, Option<u32>) {
            let page = tree.page(path).unwrap_or_else(|| panic!("the page `{path}`"));
            let embedded = texts(&page);
            match embedded.iter().find(|item| item.text == text).map(|item| &item.source) {
                Some(TextSource::Written { holder, origin }) => (holder.clone(), origin.package.clone(), origin.line),
                other => panic!("the page `{path}`: {text:?} has the source {other:?}; {embedded:?}"),
            }
        };
        let workflow = TextHolder::Workflow { name: String::from("draft") };
        assert_eq!(source_of("workflows/draft", "A draft.\n\nIt extends `text`."), (workflow.clone(), package.clone(), Some(5)));
        for path in ["workflows/draft", "workflows/", "/", "namespaces/workflow"] {
            assert_eq!(source_of(path, "A draft."), (workflow.clone(), package.clone(), Some(5)), "{path}");
        }
        let entry = TextHolder::Setting {
            feature: String::from("userdefs"),
            key: String::from("macros"),
            entry: Some(String::from("project")),
        };
        let whole = "The project.\n\nIt is written as text.";
        assert_eq!(source_of("features/userdefs/macros/project", whole), (entry.clone(), package.clone(), Some(9)));
        for path in ["features/userdefs/macros/project", "features/userdefs/macros", "features/userdefs", "namespaces/element"] {
            assert_eq!(source_of(path, "The project."), (entry.clone(), package.clone(), Some(9)), "{path}");
        }
        // The workflow that no feature declares names the package that wrote it, and links to
        // no feature's page.
        let draft = source(&tree.page("workflows/draft").unwrap(), &SourceOptions::default());
        assert!(draft.contains("\\docfield{Declared by}{written as data by the package \\docref"), "{draft}");
        assert!(!draft.contains("features/_host"), "{draft}");
    }

    /// A data extension that writes four user definitions (the entries at lines 5, 8, 11 and
    /// 14): `\amacro` and `\cmacro` with the same text, `\bmacro` with a text of two
    /// paragraphs, and `\zmacro` with a text that is the first of them.
    const TWINS: &str = "flm:\n  features:\n    userdefs:\n      macros:\n        amacro:\n          content: a\n          \
                         docs: \"Same text.\"\n        bmacro:\n          content: b\n          docs: \"Begins alike.\\n\\nMore \
                         about b.\"\n        cmacro:\n          content: c\n          docs: \"Same text.\"\n        zmacro:\n          \
                         content: z\n          docs: \"Begins alike.\"\n";

    /// A text that more than one entry of the settings holds, whole or as the first paragraph
    /// of a longer text, could come from either: it is credited to the feature that declares
    /// it, with its package, on the command's page and on the list of the feature's macros. A
    /// text that one entry alone holds comes from that entry, with the entry's origin.
    #[test]
    fn a_text_that_several_entries_hold_is_credited_to_the_feature() {
        let tree = Tree::new(&using("twins", TWINS), None);
        let sources_of = |path: &str, text: &str| -> Vec<TextSource> {
            let page = tree.page(path).unwrap_or_else(|| panic!("the page `{path}`"));
            texts(&page).into_iter().filter(|item| item.text == text).map(|item| item.source).collect()
        };
        let feature = TextSource::Feature { feature: String::from("userdefs"), package: Some(String::from("stdcore")) };
        for (path, text) in [
            ("features/userdefs/macros/amacro", "Same text."),
            ("features/userdefs/macros/cmacro", "Same text."),
            ("features/userdefs/macros", "Same text."),
            ("features/userdefs/macros/zmacro", "Begins alike."),
        ] {
            assert_eq!(sources_of(path, text), std::slice::from_ref(&feature), "the page `{path}`: {text:?}");
        }
        // `\bmacro`'s text is held by its entry alone: on its page, the text and its summary
        // come from the entry, at the place the package's origin gives (line 5, where the value
        // of `macros` begins). On the list of the macros, its summary comes from the entry, and
        // the same text, `\zmacro`'s, from the feature.
        let entry = TextHolder::Setting {
            feature: String::from("userdefs"),
            key: String::from("macros"),
            entry: Some(String::from("bmacro")),
        };
        let from_entry = |source: &TextSource| match source {
            TextSource::Written { holder, origin } => {
                *holder == entry && origin.package.as_deref() == Some("doc/_flmextensions/twins.yaml#_") && origin.line == Some(5)
            }
            _ => false,
        };
        for text in ["Begins alike.\n\nMore about b.", "Begins alike."] {
            let sources = sources_of("features/userdefs/macros/bmacro", text);
            assert!(matches!(sources.as_slice(), [source] if from_entry(source)), "{text:?}: {sources:?}");
        }
        let sources = sources_of("features/userdefs/macros", "Begins alike.");
        assert!(matches!(sources.as_slice(), [first, second] if from_entry(first) && *second == feature), "{sources:?}");
    }

    #[test]
    fn every_page_of_an_environment_with_texts_written_as_data_links_to_pages_only() {
        check_every_page(&Tree::new(&drafted(), Some(program())), "pages-drafted");
    }
}
