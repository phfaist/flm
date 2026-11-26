//! The pages over the standard environment of the library: the registry of the package
//! `std`, holding the standard features, and `math-passthrough` with a plan for the
//! workflow `text`, as `tests/describe.rs` builds it.
//!
//! The test over every page also writes each page's source under the cargo target's folder
//! for temporary files (`target/tmp/pages-core`, and `target/tmp/pages-host` under the cargo
//! feature `flm-host`), replaced at each run, so that a person can render them with the
//! program: this crate does not depend on the host and cannot render a page itself.

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{PackageEntry, PackageFactory, RegistryBuilder, RegistryInputs};
use flm_core::core::settings::PackageSettings;
use flm_core::stdlib::{self, StdlibConfig};
use flm_info::Environment;
use flm_info::pages::{
    EnvironmentVariable, ImageFolders, Output, Page, PageKind, ProgramCommand, ProgramExtensions, ProgramFolder,
    ProgramInfo, ProgramOption, ProgramPaths, SourceOptions, TextSource, Tree, source, texts,
};

/// The standard environment, with the plan of the workflow `text` in view.
fn environment() -> Environment {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder.build(&RegistryInputs::new()).expect("the standard registry builds");
    let plan = Plan::for_workflow(&registry, "text", &Overrides::none()).expect("the plan of `text`");
    let std = PackageFactory::new("std", |settings: &PackageSettings| {
        Ok(stdlib::add_features_from_settings(PackageEntry::new("std"), settings))
    })
    .expect("the factory of std");
    let factories = vec![std, flm_core::math_passthrough::factory().expect("the factory of math-passthrough")];
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
            bundled: vec![String::from("flmdoc"), String::from("math-techxt"), String::from("profile-pdf-latex"), String::from("profile-tty")],
        },
    }
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
fn the_index_lists_every_page_with_the_names_that_find_it() {
    let tree = Tree::new(&environment(), Some(program()));
    let index = tree.index();
    let walked: Vec<String> = tree.walk().map(|page| page.path).collect();
    assert_eq!(index.iter().map(|entry| entry.path.clone()).collect::<Vec<_>>(), walked);
    let names = |path: &str| index.iter().find(|entry| entry.path == path).map(|entry| entry.names.clone()).unwrap();
    assert_eq!(names("features/headings/macros/section"), ["\\section", "section"]);
    assert!(names("settings/html").contains(&String::from("html.title")));
    assert_eq!(names("features/lists/environments/itemize"), ["itemize"]);
    assert_eq!(names("features/headings"), ["headings"]);
    assert!(names("features/").is_empty());
    let section = index.iter().find(|entry| entry.path == "features/headings/macros/section").unwrap();
    assert_eq!((section.kind, section.title.as_str()), (PageKind::Command, "Macro \\section"));
    // Each name finds its page.
    for entry in &index {
        for name in &entry.names {
            assert!(tree.lookup(name).iter().any(|page| page.path == entry.path), "{name} finds {}", entry.path);
        }
    }
}

/// The path line of every page is that of the page above it, which lists it as a child,
/// and one segment more, whose path is the page's own: the levels that the paths give
/// agree with the children that the pages list. (A name that holds a slash is one segment:
/// the host's test of a package from an extension checks it.)
#[test]
fn a_page_s_path_line_is_that_of_its_parent_and_itself() {
    let tree = Tree::new(&environment(), Some(program()));
    assert!(tree.page("/").unwrap().path_line().is_empty());
    for page in tree.walk() {
        let line = page.path_line();
        for child in &page.children {
            let child_line = tree.page(&child.path).unwrap().path_line();
            let (last, above) = child_line.split_last().unwrap_or_else(|| panic!("the line of `{}`", child.path));
            assert_eq!(above, line.as_slice(), "the line of `{}` below `{}`", child.path, page.path);
            assert_eq!(last.path, child.path);
        }
    }
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

/// The program's texts are escaped on its pages, as no other test renders or reads them:
/// braces, `&` and `%` in a command's text and in an option's text.
#[test]
fn the_program_s_texts_are_escaped() {
    let tree = Tree::new(&environment(), Some(program()));
    let program = source(&tree.page("program/").expect("the page `program/`"), &SourceOptions::default());
    for written in ["Describe the environment \\{packages, features \\& more\\}", "Include the pages below in full (100\\%)"] {
        assert!(program.contains(written), "{written} in {program}");
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
    let feature = |name: &str| TextSource::Feature { feature: name.to_string(), package: Some(String::from("std")) };
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

/// The pages of an environment that the host prepared (cargo feature `flm-host`): a
/// document in memory that uses a data extension found in the folder `_flmextensions`
/// beside it and writes a setting, as `tests/host.rs` prepares it.
#[cfg(feature = "flm-host")]
mod host {
    use std::sync::Arc;

    use flm_core::progress::Scope;
    use flm_info::Environment;
    use flm_info::pages::{SourceOptions, TextHolder, TextSource, Tree, source, texts};
    use flm_host::files::MemoryFiles;
    use flm_host::packages::ExtensionFolder;
    use flm_host::seed::MathChoice;
    use flm_host::session::{Input, Request, prepare};

    use super::{check_every_page, program};

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
        let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
        request.default_math = MathChoice::processor("passthrough");
        request.profile = Some(String::from("text"));
        request.extension_folders = vec![ExtensionFolder { path: String::from("doc/_flmextensions"), required: false }];
        let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("the document is prepared");
        Environment::of_prepared(&prepared)
    }

    /// The host's pages show the profile in view on `/`, a package from an extension as
    /// such, the value in effect of a setting, and a feature switched off as missing from the
    /// registry; the package's name, which holds slashes, is one segment of its path line.
    #[test]
    fn the_host_s_pages_show_the_profile_the_origins_and_the_settings_in_effect() {
        let env = environment();
        let tree = Tree::new(&env, None);
        let source_of = |path: &str| source(&tree.page(path).unwrap_or_else(|| panic!("the page `{path}`")), &SourceOptions::default());
        let root = source_of("/");
        assert!(root.contains("\\docfield{Profile in view}{\\verbcode{text}}"), "{root}");
        assert!(root.contains("origin=extension, extension={\\verbcode{doc/_flmextensions/narrow.yaml}}"), "{root}");
        let settings = source_of("settings/text");
        assert!(settings.contains("\\docfield{Value in effect}{\\verbcode{30}}"), "{settings}");
        // A feature switched off has no page; its package's page does not link to it.
        assert!(tree.page("features/toc").is_none());
        let std = source_of("packages/std");
        assert!(std.contains("\\item[missing] \\verbcode{toc}"), "{std}");
        // The package's name holds slashes, and is one segment of its page's path line.
        let package = tree.page("packages/doc/_flmextensions/narrow.yaml#_").unwrap();
        let paths: Vec<String> = package.path_line().into_iter().map(|segment| segment.path).collect();
        assert_eq!(paths, ["packages/", "packages/doc/_flmextensions/narrow.yaml#_"]);
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
        let mut request = Request::new(Input::File(String::from("doc/draft.flm")));
        request.default_math = MathChoice::processor("passthrough");
        request.extension_folders = vec![ExtensionFolder { path: String::from("doc/_flmextensions"), required: false }];
        let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("the document is prepared");
        Environment::of_prepared(&prepared)
    }

    /// A text written as data comes from where it was written, both whole and as its
    /// summary, on every page that embeds it: the workflow's on its page, on the list of
    /// workflows, on `/` and on the list of the names of the namespace `workflow`; the user
    /// definition's on its page, on the list of the feature's macros and on the page of its
    /// element type (its element value). The list of the names of the namespace `element`
    /// shows the element type's own text, not the value's.
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
        for path in ["features/userdefs/macros/project", "features/userdefs/macros", "namespaces/element/flm.userdefs.macro.project"] {
            assert_eq!(source_of(path, "The project."), (entry.clone(), package.clone(), Some(9)), "{path}");
        }
        let names = texts(&tree.page("namespaces/element").expect("the page `namespaces/element`"));
        assert!(!names.iter().any(|item| item.text == "The project."), "{names:?}");
        // The workflow that no feature declares names the package that wrote it, and links to
        // no feature's page.
        let draft = source(&tree.page("workflows/draft").unwrap(), &SourceOptions::default());
        assert!(draft.contains("declared-by={\\begin{docinlinelist} \\item[package] \\docref"), "{draft}");
        assert!(draft.contains("declarer=data]{workflow}"), "{draft}");
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
        let feature = TextSource::Feature { feature: String::from("userdefs"), package: Some(String::from("std")) };
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
