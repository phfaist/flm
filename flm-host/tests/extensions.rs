//! Extensions of the data engine, in memory: found by their locator on the search path in
//! either layout, their settings, workflows and templates in force, the ranks of the sources
//! of settings, and the problems of finding, loading and combining packages. No test
//! touches the disk. Every manifest and document is written through the forms of
//! `support/files.rs`, which check it against the schema of its kind.

use std::sync::{Arc, Mutex};

use flm_core::core::registry::{PackageEntry, PackageFactory};
use flm_core::progress::Scope;
use flm_host::files::{FileError, Files, MemoryFiles};
use flm_host::output::{MemorySink, Sink};
use flm_host::packages::ExtensionFolder;
use flm_host::report::{ContentOffsets, Place, Record, Reported};
use flm_host::session::{Built, Input, PrepareError, PrepareErrorKind, Prepared, Request, build, prepare};
use flm_host::settings::{CommandLineSetting, UnknownFeature};
use flm_core::techy::error::DiagnosticInfo;

#[path = "support/files.rs"]
mod support;
use support::Written;

#[path = "support/diagnostics.rs"]
mod diagnostics;
use diagnostics::assert_only_unchecked_output;

/// A paragraph of short words, which the text output wraps at the width in force.
const WORDS: &str = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen\n";

/// The document `doc/paper.flm` using `uses`, with `more` added under `flm`, then `WORDS`.
fn paper(uses: &str, more: &str) -> String {
    format!("---\nflm:\n  use: [{uses}]\n{more}---\n{WORDS}")
}

/// A block setting the text output's width to `width`.
fn width(width: u32) -> String {
    format!("features:\n  text:\n    width: {width}\n")
}

/// A manifest of the data engine whose block `flm` is `block`: each line of `block` one
/// line below `flm:`, indented.
fn flm(block: &str) -> String {
    format!("flm:\n{}", block.lines().map(|line| format!("  {line}\n")).collect::<String>())
}

/// A request for `doc/paper.flm`, workflow `text`, looking for extensions in `folders`.
fn request(folders: &[&str]) -> Request {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request.extension_folders =
        folders.iter().map(|folder| ExtensionFolder { path: String::from(*folder), required: false }).collect();
    request
}

/// Whether `path` is one of the paths where the extension of the request's profile `text`
/// is looked for, which every preparation reads.
fn probe(path: &str) -> bool {
    path.starts_with("defs/profile-text")
}

fn prepared(files: MemoryFiles, request: Request) -> Result<Prepared, PrepareError> {
    prepare(request, Arc::new(files), &[], &Scope::none())
}

/// The text output of `request` over `files`, and the build.
fn rendered(files: MemoryFiles, mut request: Request) -> (Built, String) {
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let built = build(&prepared(files, request).expect("prepared"), &Scope::none()).expect("built");
    let entry = sink.files().into_values().find(|file| file.entry).expect("an output");
    (built, String::from_utf8(entry.bytes).unwrap())
}

/// The longest line of the text output of `request` over `files`: the width in force.
fn widest(files: MemoryFiles, request: Request) -> usize {
    rendered(files, request).1.lines().map(|line| line.chars().count()).max().unwrap_or(0)
}

/// Every problem of a preparation that failed, as `kind`, printed as `file:line: detail`.
fn problems(files: MemoryFiles, request: Request, kind: PrepareErrorKind) -> Vec<String> {
    let error = prepared(files, request).expect_err("a problem");
    assert_eq!(error.kind, kind, "{error:?}");
    error
        .problems
        .iter()
        .map(|problem| {
            let line = problem.place.map(|place| place.line.to_string()).unwrap_or_default();
            format!("{}:{line}: {}", problem.file.clone().unwrap_or_default(), problem.detail)
        })
        .collect()
}

// --- 1. a value set by an extension is in force ------------------------------------------

#[test]
fn a_file_extension_and_a_folder_extension_set_values() {
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("narrow", "")).extension("defs/narrow.yaml", &flm(&width(20)));
    assert!(widest(files, request(&["defs"])) <= 20);
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("narrow", ""))
        .extension("defs/narrow/flmextension.yaml", &flm(&width(20)));
    assert!(widest(files, request(&["defs"])) <= 20);
}

#[test]
fn the_earlier_folder_of_the_search_path_wins() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("narrow, other", ""))
        .extension("first/narrow.yaml", &flm(&width(20)))
        .extension("second/narrow.yaml", &flm(&width(30)))
        .extension("third/other.yaml", "extension: {description: found in the third folder}\n");
    assert!(widest(files, request(&["first", "second", "third"])) <= 20);
}

#[test]
fn the_engine_is_data_when_absent_and_an_empty_manifest_is_an_extension() {
    for manifest in ["", "extension: {}\n"] {
        let files = MemoryFiles::new().document("doc/paper.flm", &paper("e", "")).extension("defs/e.yaml", manifest);
        let prepared = prepared(files, request(&["defs"])).expect("prepared");
        assert!(prepared.packages().iter().any(|name| name == "defs/e.yaml#_"), "{:?}", prepared.packages());
    }
}

#[test]
fn a_nested_locator_is_found_below_a_folder_of_the_search_path() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("\"@acme/style\"", ""))
        .extension("_flmextensions/@acme/style.yaml", &flm(&width(20)));
    assert!(widest(files, request(&["_flmextensions"])) <= 20);
}

#[test]
fn an_extension_and_its_default_package_are_one_package() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("myext, \"myext#_\"", ""))
        .extension("defs/myext.yaml", &flm(&width(20)));
    let prepared = prepared(files.clone(), request(&["defs"])).expect("prepared");
    assert_eq!(prepared.packages().iter().filter(|name| name.contains("myext")).count(), 1, "{:?}", prepared.packages());
    let read = prepared.files_read().iter().filter(|read| read.path == "defs/myext.yaml").count();
    assert_eq!(read, 1);
    assert!(widest(files, request(&["defs"])) <= 20);
}

#[test]
fn the_initial_list_comes_before_the_document_s_use() {
    // Neither `first` nor `second` uses a package, so only the order of the two lists places
    // `first` before `second`.
    let files = MemoryFiles::new()
        .document("doc/paper.flm", "---\nflm:\n  initial_use: [std, first]\n  use: [second]\n---\nHello.\n")
        .extension("defs/first.yaml", &flm("use: []\n"))
        .extension("defs/second.yaml", &flm("use: []\n"));
    let prepared = prepared(files, request(&["defs"])).expect("prepared");
    let packages = prepared.packages();
    let position = |wanted: &str| packages.iter().position(|name| name.contains(wanted)).expect("used");
    assert_eq!(packages.len(), 4, "{packages:?}");
    assert!(position("first") < position("second"), "{packages:?}");
}

// --- 2. a compiled package is never read from a file --------------------------------------

#[test]
fn a_file_named_as_a_compiled_package_is_not_read() {
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("std", "")).faulty("defs/std.yaml", "not: [valid");
    let prepared = prepared(files, request(&["defs"])).expect("the compiled package");
    assert!(prepared.files_read().iter().all(|read| !read.path.starts_with("defs") || probe(&read.path)), "{:?}", prepared.files_read());
}

// --- 3. the packages a package uses; loops; read once -----------------------------------

#[test]
fn an_extension_brings_the_extensions_it_uses_each_read_once_loops_included() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("outer, inner", ""))
        .extension("defs/outer.yaml", &flm("use: [inner]\n"))
        .extension("defs/inner.yaml", &flm(&format!("use: [outer]\n{}", width(20))));
    let prepared = prepared(files.clone(), request(&["defs"])).expect("a loop builds");
    let inner = prepared.files_read().iter().filter(|read| read.path == "defs/inner.yaml").count();
    assert_eq!(inner, 1, "{:?}", prepared.files_read());
    assert_eq!(prepared.packages().iter().filter(|name| name.contains("inner")).count(), 1);
    assert!(widest(files, request(&["defs"])) <= 20);
}

// --- 4. the ranks -------------------------------------------------------------------------

#[test]
fn each_source_wins_over_every_source_below_it() {
    let package = MemoryFiles::new().extension("defs/narrow.yaml", &flm(&width(40)));
    let with = |document: String| package.clone().document("doc/paper.flm", &document);
    assert!(widest(with(paper("narrow", "")), request(&["defs"])) <= 40);
    let front = "  features:\n    text:\n      width: 30\n";
    assert!(widest(with(paper("narrow", front)), request(&["defs"])) <= 30);
    let workflow = MemoryFiles::new()
        .extension("defs/narrow.yaml", &flm(&format!("{}workflows:\n  text:\n    settings:\n      text: {{width: 20}}\n", width(40))))
        .document("doc/paper.flm", &paper("narrow", front));
    assert!(widest(workflow.clone(), request(&["defs"])) <= 20);
    let mut command_line = request(&["defs"]);
    command_line.options.push(CommandLineSetting::new("text.width=15", "-O text.width=15"));
    assert!(widest(workflow, command_line) <= 15);
}

#[test]
fn a_package_replaces_a_default_of_the_program_and_the_front_matter_replaces_the_package_s_value() {
    // The program's defaults mark a footnote with a letter; the package marks it with a
    // number, the front matter with a Roman numeral. The entry `footnote` is written whole,
    // as a value of `xnotes.kinds` is today.
    let note = |more: &str| format!("---\nflm:\n  use: [notes]\n{more}---\nA\\footnote{{note}}.\n");
    let kinds = |indent: &str, mark: &str| {
        format!(
            "{indent}xnotes:\n{indent}  kinds:\n{indent}    footnote:\n{indent}      commands: [footnote]\n\
             {indent}      words: {{base: {{one: Note~, other: Notes~}}}}\n{indent}      mark: {{formats: '{mark}'}}\n"
        )
    };
    let files = |more: &str| {
        MemoryFiles::new()
            .document("doc/paper.flm", &note(more))
            .extension("defs/notes.yaml", &flm(&format!("features:\n{}", kinds("  ", "#{arabic}"))))
    };
    let (_, bare) = rendered(MemoryFiles::new().document("doc/paper.flm", "A\\footnote{note}.\n"), request(&[]));
    let (_, package) = rendered(files(""), request(&["defs"]));
    let (_, front) = rendered(files(&format!("  features:\n{}", kinds("    ", "#{roman}"))), request(&["defs"]));
    assert!(bare.starts_with("Aᵃ."), "{bare}");
    assert!(package.starts_with("A¹."), "{package}");
    assert!(front.starts_with("Aⁱ."), "{front}");
}

// --- 5. two packages ----------------------------------------------------------------------

#[test]
fn two_packages_combine_agree_or_are_reported_where_they_disagree() {
    let base = || MemoryFiles::new().document("doc/paper.flm", &paper("a, b", ""));
    let agree = base()
        .extension("defs/a.yaml", &flm(&width(20)))
        .extension("defs/b.yaml", &flm(&format!("{}parsing: {{comments: true}}\n", width(20))));
    assert!(widest(agree, request(&["defs"])) <= 20);
    let disagree = base()
        .extension("defs/a.yaml", &flm(&width(20)))
        .extension("defs/b.yaml", &format!("extension: {{description: B}}\n{}", flm(&width(30))));
    let found = problems(disagree, request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/a.yaml:4:"), "{found:?}");
    let settled = base()
        .document("doc/paper.flm", &paper("a, b", "  features:\n    text: {width: 25}\n"))
        .extension("defs/a.yaml", &flm(&width(20)))
        .extension("defs/b.yaml", &flm(&width(30)));
    assert!(widest(settled, request(&["defs"])) <= 25);
}

// --- 5b. a package wins over the packages it uses ---------------------------------------

/// A manifest whose block uses `uses` (a list, possibly empty) and writes `rest`.
fn using(uses: &str, rest: &str) -> String {
    flm(&format!("use: [{uses}]\n{rest}"))
}

/// The footnote kind, its mark written with `mark`, under `features:`.
fn footnote(mark: &str) -> String {
    format!(
        "  xnotes:\n    kinds:\n      footnote:\n        commands: [footnote]\n\
         \x20       words: {{base: {{one: Note~, other: Notes~}}}}\n        mark: {{formats: '{mark}'}}\n"
    )
}

#[test]
fn a_package_wins_over_the_packages_it_uses_whatever_the_order_of_the_use_list() {
    let base = |uses: &str| {
        MemoryFiles::new()
            .document("doc/paper.flm", &paper(uses, ""))
            .extension("defs/article.yaml", &flm(&width(40)))
            .extension("defs/my-article.yaml", &using("article", &width(20)))
    };
    for uses in ["article, my-article", "my-article, article"] {
        let (built, text) = rendered(base(uses), request(&["defs"]));
        assert!(!built.has_errors(), "{:?}", built.diagnostics);
        assert!(text.lines().map(|line| line.chars().count()).max().unwrap_or(0) <= 20, "{uses}: {text}");
    }
    // The value in force is the using package's, at its place: a peer of `my-article` that
    // writes another value is reported against it.
    let other = base("article, my-article, other").extension("defs/other.yaml", &flm(&width(25)));
    let found = problems(other, request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/my-article.yaml:5:"), "{found:?}");
}

#[test]
fn a_package_wins_over_the_packages_it_uses_through_others() {
    // `a` uses `b`, which uses `c`: all three write the width, `b` and `c` the footnote.
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &format!("---\nflm:\n  use: [a]\n---\n{WORDS}A\\footnote{{note}}.\n"))
        .extension("defs/a.yaml", &using("b", &width(20)))
        .extension("defs/b.yaml", &using("c", &format!("{}{}", width(30), footnote("#{roman}"))))
        .extension("defs/c.yaml", &using("", &format!("{}{}", width(40), footnote("#{arabic}"))));
    let (built, text) = rendered(files, request(&["defs"]));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.lines().map(|line| line.chars().count()).max().unwrap_or(0) <= 20, "{text}");
    assert!(text.contains("Aⁱ."), "{text}");
}

#[test]
fn packages_of_which_neither_uses_the_other_are_peers_and_one_using_one_of_them_takes_its_place() {
    let base = |uses: &str| {
        MemoryFiles::new()
            .document("doc/paper.flm", &paper(uses, ""))
            .extension("defs/a.yaml", &flm(&width(20)))
            .extension("defs/c.yaml", &flm(&width(30)))
    };
    let found = problems(base("a, c"), request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/a.yaml:4:"), "{found:?}");
    // `d` uses `a` and writes `c`'s value: they agree, and `a`'s value no longer counts.
    let agree = base("d, c").extension("defs/d.yaml", &using("a", &width(30)));
    assert!(widest(agree, request(&["defs"])) <= 30);
    // `d` writes a third value: `d` and `c` disagree.
    let disagree = base("d, c").extension("defs/d.yaml", &using("a", &width(25)));
    let found = problems(disagree, request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/d.yaml:5:"), "{found:?}");
}

#[test]
fn packages_that_use_each_other_directly_or_through_a_third_are_peers() {
    let direct = MemoryFiles::new()
        .document("doc/paper.flm", &paper("a", ""))
        .extension("defs/a.yaml", &using("b", &width(20)))
        .extension("defs/b.yaml", &using("a", &width(30)));
    let through = MemoryFiles::new()
        .document("doc/paper.flm", &paper("a", ""))
        .extension("defs/a.yaml", &using("b", &width(20)))
        .extension("defs/b.yaml", &using("c", ""))
        .extension("defs/c.yaml", &using("a", &width(30)));
    for (files, other) in [(direct, "defs/b.yaml"), (through, "defs/c.yaml")] {
        let found = problems(files, request(&["defs"]), PrepareErrorKind::Settings);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with(&format!("{other}:5:")), "{found:?}");
    }
}

#[test]
fn in_a_loop_of_use_the_package_that_reaches_the_other_gives_a_workflow_s_fields() {
    // `x` and `z` use each other; `x` reaches `y` through `z`, and `y` reaches neither: `x`'s
    // outputs are in force, whatever the order of `z`'s list.
    let flow = |outputs: &str| format!("workflows:\n  mine:\n    extends: text\n    outputs: [{outputs}]\n");
    for z_uses in ["x, y", "y, x"] {
        let files = MemoryFiles::new()
            .document("doc/paper.flm", &paper("z", ""))
            .extension("defs/z.yaml", &using(z_uses, ""))
            .extension("defs/x.yaml", &using("z", &flow("text")))
            .extension("defs/y.yaml", &flm(&flow("html")));
        let mut chosen = request(&["defs"]);
        chosen.profile = Some(String::from("mine"));
        let ready = prepared(files.clone(), chosen.clone()).expect("prepared");
        assert_eq!(ready.outputs(), ["text"], "`z` uses [{z_uses}]");
        assert!(ready.diagnostics().is_empty(), "{:?}", ready.diagnostics());
        let (built, text) = rendered(files, chosen);
        assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
        assert!(text.starts_with("one two"), "{text}");
    }
}

#[test]
fn a_package_wins_over_the_packages_it_uses_for_an_entry_a_switch_and_a_workflow() {
    let document = "---\nflm:\n  use: [my-article]\n---\nA\\footnote{x} B\\aside{y}.\n";
    let with = |article: &str, mine: &str| {
        MemoryFiles::new()
            .document("doc/paper.flm", document)
            .extension("defs/article.yaml", &flm(article))
            .extension("defs/my-article.yaml", &using("article", mine))
    };
    // One entry: `my-article` replaces `article`'s footnote; `article`'s aside stays.
    let aside = "      aside:\n        commands: [aside]\n        words: {base: {one: Aside~, other: Asides~}}\n";
    let entries = with(&format!("features:\n{}{aside}", footnote("#{arabic}")), &format!("features:\n{}", footnote("#{roman}")));
    let (built, text) = rendered(entries, request(&["defs"]));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(text.starts_with("Aⁱ B¹."), "{text}");
    // A feature's switch, both ways.
    let switched = |article: &str, mine: &str| {
        let files = with(article, mine);
        build(&prepared(files, request(&["defs"])).expect("prepared"), &Scope::none()).expect("built").has_errors()
    };
    assert!(!switched("features:\n  xnotes: false\n", &format!("features:\n{}{aside}", footnote("#{roman}"))));
    assert!(switched(&format!("features:\n{}{aside}", footnote("#{roman}")), "features:\n  xnotes: false\n"));
    // A workflow's render settings, and one of its fields.
    let flows = |width: u32, outputs: &str| {
        format!("workflows:\n  text:\n    settings:\n      text: {{width: {width}}}\n  mine:\n    extends: text\n    outputs: [{outputs}]\n")
    };
    let workflows = with(&flows(40, "html"), &flows(20, "text")).document("doc/paper.flm", &paper("my-article", ""));
    let chosen = prepared(workflows.clone(), request(&["defs"])).expect("prepared");
    let mine = chosen.registry().combined_workflow("mine").expect("the workflow");
    assert_eq!(mine.outputs.iter().map(|output| output.as_str()).collect::<Vec<_>>(), ["text"]);
    assert!(widest(workflows, request(&["defs"])) <= 20);
}

// --- 6. a feature switched off ------------------------------------------------------------

#[test]
fn a_feature_switched_off_by_a_package_is_switched_on_by_the_front_matter() {
    let note = "---\nflm:\n  use: [quiet]\n{more}---\nA\\footnote{note}.\n";
    let off = "features:\n  xnotes: false\n";
    let files = |more: &str| {
        MemoryFiles::new().document("doc/paper.flm", &note.replace("{more}", more)).extension("defs/quiet.yaml", &flm(off))
    };
    let built = |files: MemoryFiles| build(&prepared(files, request(&["defs"])).expect("prepared"), &Scope::none())
        .expect("built");
    assert!(built(files("")).has_errors(), "the footnote is unknown with the feature off");
    let built = built(files("  features:\n    xnotes: {}\n"));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let peers = MemoryFiles::new()
        .document("doc/paper.flm", &paper("quiet, loud", ""))
        .extension("defs/quiet.yaml", &flm(off))
        .extension("defs/loud.yaml", &flm("features:\n  xnotes: {}\n"));
    let found = problems(peers, request(&["defs"]), PrepareErrorKind::Settings);
    assert!(found[0].starts_with("defs/quiet.yaml:3:"), "{found:?}");
}

#[test]
fn the_key_enabled_switches_a_feature_off_and_keeps_its_settings_as_the_source_s_values() {
    // A package switches the text output's feature off and writes its width: the feature is
    // off, at the line of the key `enabled`, and no key of it is in effect.
    let quiet = flm("features:\n  text:\n    width: 20\n    enabled: false\n");
    let files = |more: &str| {
        MemoryFiles::new().document("doc/paper.flm", &paper("quiet", more)).extension("defs/quiet.yaml", &quiet)
    };
    let mut html = request(&["defs"]);
    html.profile = Some(String::from("html"));
    let off = prepared(files(""), html).expect("prepared");
    let text = &off.settings_in_effect()["text"];
    let switched_off = text.switched_off.as_ref().expect("switched off");
    assert_eq!(switched_off.place.map(|place| place.line), Some(5));
    assert!(text.keys.is_empty(), "{text:?}");
    // `enabled: true` in the front matter switches it on, with the package's width.
    let on = files("  features:\n    text: {enabled: true}\n");
    let width = &prepared(on.clone(), request(&["defs"])).expect("prepared").settings_in_effect()["text"].keys["width"];
    assert_eq!(width.value, serde_json::json!(20));
    assert_eq!(width.origin.place.map(|place| place.line), Some(4));
    assert!(widest(on, request(&["defs"])) <= 20);
    // `false` and `{enabled: false}` read as one.
    let shorthand = |entry: &str| {
        let files = MemoryFiles::new().document("doc/paper.flm", &paper("", &format!("  features:\n    xnotes: {entry}\n")));
        prepared(files, request(&["defs"])).expect("prepared").settings_in_effect()["xnotes"].switched_off.is_some()
    };
    assert!(shorthand("false") && shorthand("{enabled: false}"));
}

#[test]
fn a_workflow_s_settings_cannot_write_the_key_enabled() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("flows", ""))
        .refused_extension("defs/flows.yaml", &flm("workflows:\n  text:\n    settings:\n      text: {enabled: true, width: 20}\n"));
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/flows.yaml:5:") && found[0].contains("‘enabled’"), "{found:?}");
}

// --- 7. workflows -------------------------------------------------------------------------

#[test]
fn a_workflow_a_package_defines_is_chosen_and_one_it_names_after_a_standard_one_changes_it() {
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("flows", "")).extension(
        "defs/flows.yaml",
        &flm("workflows:\n  narrow:\n    extends: text\n    settings:\n      text: {width: 20}\n    docs: Narrow text.\n"),
    );
    let mut chosen = request(&["defs"]);
    chosen.profile = Some(String::from("narrow"));
    assert!(widest(files.clone(), chosen) <= 20);
    assert!(widest(files, request(&["defs"])) > 20);
    let changed = MemoryFiles::new()
        .document("doc/paper.flm", &paper("flows", ""))
        .extension("defs/flows.yaml", &flm("workflows:\n  text:\n    settings:\n      text: {width: 20}\n"));
    let prepared = prepared(changed.clone(), request(&["defs"])).expect("prepared");
    assert_eq!(prepared.registry().workflows().filter(|entry| entry.def.name.as_str() == "text").count(), 1);
    assert!(widest(changed, request(&["defs"])) <= 20);
}

#[test]
fn two_packages_writing_one_field_of_a_workflow_differently_are_reported() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("a, b", ""))
        .extension("defs/a.yaml", &flm("workflows:\n  text: {docs: A}\n"))
        .extension("defs/b.yaml", &flm("workflows:\n  text: {docs: B}\n"));
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/a.yaml:3:"), "{found:?}");
}

// --- 8. templates -------------------------------------------------------------------------

/// The description of the compose template `name` for the output `text`, its text `page.txt`.
fn marked(name: &str) -> String {
    format!("name: {name}\noutput: text\nengine: minijinja\nmain: page.txt\naccepts: all\n")
}

#[test]
fn a_template_a_folder_extension_ships_is_chosen_by_name() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("marked", ""))
        .extension("defs/marked/flmextension.yaml", "extension: {description: A template between markers.}\n")
        .with("defs/marked/templates/text/marked/flmtemplate.yaml", &marked("marked"))
        .with("defs/marked/templates/text/marked/page.txt", "BEGIN\n{{ regions.content }}\nEND\n");
    let mut chosen = request(&["defs"]);
    chosen.options.push(CommandLineSetting::new("compose.template='marked'", "-t marked"));
    let (_, text) = rendered(files, chosen);
    assert!(text.starts_with("BEGIN\none two"), "{text}");
}

#[test]
fn the_description_titles_the_templates_feature() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("marked", ""))
        .extension("defs/marked/flmextension.yaml", "extension: {description: A template between markers.}\n")
        .with("defs/marked/templates/text/marked/flmtemplate.yaml", &marked("marked"))
        .with("defs/marked/templates/text/marked/page.txt", "{{ regions.content }}");
    let prepared = prepared(files, request(&["defs"])).expect("prepared");
    let feature = prepared.registry().feature("defs/marked/flmextension.yaml#_").expect("the templates feature");
    assert_eq!(feature.title, "A template between markers.");
}

#[test]
fn two_templates_of_one_name_for_one_output_are_reported_when_the_settings_are_read() {
    // Each package declares the settings of its `simple`: one entry, declared twice.
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("marked", ""))
        .extension("defs/marked/flmextension.yaml", "")
        .with("defs/marked/templates/text/simple/flmtemplate.yaml", &marked("simple"))
        .with("defs/marked/templates/text/simple/page.txt", "{{ regions.content }}");
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("‘std’") && found[0].contains("marked"), "{found:?}");
}

#[test]
fn a_file_among_the_outputs_of_an_extension_s_templates_is_left_out() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("marked", ""))
        .extension("defs/marked/flmextension.yaml", "")
        .with("defs/marked/templates/README", "The templates.\n")
        .with("defs/marked/templates/text/marked/flmtemplate.yaml", &marked("marked"))
        .with("defs/marked/templates/text/marked/page.txt", "BEGIN\n{{ regions.content }}\nEND\n");
    let mut chosen = request(&["defs"]);
    chosen.options.push(CommandLineSetting::new("compose.template='marked'", "-t marked"));
    let (_, text) = rendered(files, chosen);
    assert!(text.starts_with("BEGIN\none two"), "{text}");
}

#[test]
fn every_problem_of_an_extension_s_templates_is_reported() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("marked", ""))
        .extension("defs/marked/flmextension.yaml", "")
        .with("defs/marked/templates/text/broken/flmtemplate.yaml", "output: [text\n")
        .with("defs/marked/templates/text/broken/page.txt", "{{ regions.content }}")
        .with("defs/marked/templates/text/empty.yaml", "name: empty\n");
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 2, "{found:?}");
    // Placed where the document lists the extension.
    assert!(found.iter().all(|problem| problem.starts_with("doc/paper.flm:3:")), "{found:?}");
}

// --- 9. the files read --------------------------------------------------------------------

#[test]
fn every_file_of_an_extension_read_is_on_the_list_of_files_read() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("outer", ""))
        .extension("defs/outer/flmextension.yaml", &flm("use: [inner]\n"))
        .with("defs/outer/templates/text/marked/flmtemplate.yaml", &marked("marked"))
        .with("defs/outer/templates/text/marked/page.txt", "{{ regions.content }}")
        .extension("defs/inner.yaml", "extension: {description: Inner.}\n");
    let prepared = prepared(files, request(&["defs"])).expect("prepared");
    let found: Vec<String> =
        prepared.files_read().iter().filter(|read| read.found).map(|read| read.path.clone()).collect();
    for path in [
        "defs/outer/flmextension.yaml",
        "defs/outer/templates",
        "defs/outer/templates/text",
        "defs/outer/templates/text/marked",
        "defs/outer/templates/text/marked/flmtemplate.yaml",
        "defs/outer/templates/text/marked/page.txt",
        "defs/inner.yaml",
    ] {
        assert!(found.iter().any(|found| found == path), "{path}: {found:?}");
    }
}

// --- 10. the problems of finding, loading and combining --------------------------------

#[test]
fn the_problems_of_finding_an_extension_are_reported_where_its_entry_is_written() {
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("nosuch", "")).extension("defs/outer.yaml", &flm("use: [absent]\n"));
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:3:"), "{found:?}");
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("outer", "")).extension("defs/outer.yaml", &flm("use: [absent]\n"));
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert!(found[0].starts_with("defs/outer.yaml:2:"), "{found:?}");
    // Met twice: from the document's own entry, and through the extension `c`.
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("c, nosuch", "")).extension("defs/c.yaml", &flm("use: nosuch\n"));
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().any(|problem| problem.starts_with("doc/paper.flm:3:")), "{found:?}");
    assert!(found.iter().any(|problem| problem.starts_with("defs/c.yaml:2:")), "{found:?}");
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("both", ""))
        .extension("defs/both.yaml", "")
        .extension("defs/both/flmextension.yaml", "");
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:3:"), "{found:?}");
    let mut required = request(&[]);
    required.extension_folders.push(ExtensionFolder { path: String::from("nosuch"), required: true });
    let found = problems(MemoryFiles::new().document("doc/paper.flm", "x"), required, PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("::"), "{found:?}");
    let mut unreadable = MemoryFiles::new().document("doc/paper.flm", &paper("bytes", ""));
    unreadable.insert("defs/bytes.yaml", vec![0xff, 0xfe]);
    let found = problems(unreadable, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/bytes.yaml::"), "{found:?}");
}

#[test]
fn a_package_the_extension_lacks_is_reported_at_the_entry() {
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("myext#svg", "")).extension("defs/myext.yaml", "");
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:3:"), "{found:?}");
}

#[test]
fn a_path_in_use_is_refused_where_it_is_written() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("./shared/mystyle", ""))
        .extension("doc/shared/mystyle.yaml", &flm(&width(20)));
    let found = problems(files, request(&[]), PrepareErrorKind::FrontMatter);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:3:"), "{found:?}");
}

#[test]
fn a_dot_or_a_colon_in_use_is_refused_where_it_is_written() {
    let found = problems(MemoryFiles::new().document("doc/paper.flm", &paper(".#core", "")), request(&[]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:3:"), "{found:?}");
    let found = problems(MemoryFiles::new().document("doc/paper.flm", &paper("file:x", "")), request(&[]), PrepareErrorKind::FrontMatter);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:3:"), "{found:?}");
}

#[test]
fn a_front_matter_must_not_define_workflows() {
    let files = MemoryFiles::new().refused_document("doc/paper.flm", &paper("", "  workflows: {}\n"));
    let found = problems(files, request(&[]), PrepareErrorKind::FrontMatter);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("doc/paper.flm:4:"), "{found:?}");
}

#[test]
fn a_manifest_that_names_an_engine_this_program_lacks_is_reported() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("x", ""))
        .refused_extension("defs/x.yaml", "extension: {engine: python}\n");
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/x.yaml:1:"), "{found:?}");
}

#[test]
fn an_unmet_flm_version_is_reported() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("x", ""))
        .extension("defs/x.yaml", "extension: {flm_version: \">=999\"}\n");
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/x.yaml:1:"), "{found:?}");
}

#[test]
fn the_problems_of_a_manifest_are_placed_in_its_file() {
    // `write` is the form the manifest `defs/p.yaml` is written in.
    let read = |write: fn(MemoryFiles, &str, &str) -> MemoryFiles, text: &str, kind: PrepareErrorKind| {
        let files = write(MemoryFiles::new().document("doc/paper.flm", &paper("p", "")), "defs/p.yaml", text);
        problems(files, request(&["defs"]), kind)
    };
    let (faulty, refused, extension) = (Written::faulty, Written::refused_extension, Written::extension);
    let found = read(faulty, "a: [b\n", PrepareErrorKind::Packages);
    assert!(found[0].starts_with("defs/p.yaml:2:"), "{found:?}");
    // A problem of the manifest's shape is placed in the file.
    assert!(read(refused, "- a\n", PrepareErrorKind::Packages)[0].starts_with("defs/p.yaml::"));
    assert!(read(refused, "docs: x\n", PrepareErrorKind::Packages)[0].starts_with("defs/p.yaml:1:"));
    let found = read(refused, &flm("layout: blocks\n"), PrepareErrorKind::Packages);
    assert!(found[0].starts_with("defs/p.yaml:2:"), "{found:?}");
    // `initial_use` belongs to a document's front matter: elsewhere it is a key like any
    // other that the place does not admit.
    let found = read(refused, &flm("initial_use: [std]\n"), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/p.yaml:2:"), "{found:?}");
    let found = read(refused, &flm("workflows:\n  mine: {outputs: [text], initial_use: [std]}\n"), PrepareErrorKind::Packages);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/p.yaml:3:"), "{found:?}");
    let found = read(refused, &flm("use: 3\n"), PrepareErrorKind::Packages);
    assert!(found[0].starts_with("defs/p.yaml:2:"), "{found:?}");
    assert!(read(refused, &flm("workflows:\n  mine: {outputs: text}\n"), PrepareErrorKind::Packages)[0].starts_with("defs/p.yaml:3:"));
    // A value the settings refuse is placed at its line.
    assert!(read(refused, &flm("features:\n  text:\n    wdth: 3\n"), PrepareErrorKind::Settings)[0].starts_with("defs/p.yaml:4:"));
    assert!(read(refused, &flm("features:\n  text:\n    width: many\n"), PrepareErrorKind::Settings)[0].starts_with("defs/p.yaml:4:"));
    // The schema of a workflow's settings admits every key; the reader only those set per render.
    let workflow = flm("workflows:\n  text:\n    settings:\n      xnotes: {kinds: {}}\n");
    assert!(read(faulty, &workflow, PrepareErrorKind::Settings)[0].starts_with("defs/p.yaml:5:"));
    let found = read(extension, &flm("workflows:\n  mine: {extends: nosuch}\n"), PrepareErrorKind::Registry);
    assert!(found[0].starts_with("defs/p.yaml:3:"), "{found:?}");
    let found = read(extension, &flm("workflows:\n  mine:\n    outputs: [text, nosuch]\n"), PrepareErrorKind::Registry);
    assert!(found[0].starts_with("defs/p.yaml:4:"), "{found:?}");
}

/// A compiled package, to show that the finder is asked only for the names no compiled
/// package carries.
#[test]
fn a_compiled_package_the_program_carries_wins_over_a_file_of_its_name() {
    let compiled = PackageFactory::new("mine", |_| Ok(PackageEntry::new("mine"))).unwrap();
    let files = MemoryFiles::new().document("doc/paper.flm", &paper("mine", "")).faulty("defs/mine.yaml", "not: [valid");
    let prepared = prepare(request(&["defs"]), Arc::new(files), &[compiled], &Scope::none())
        .expect("the compiled package");
    assert!(prepared.packages().iter().any(|name| name == "mine"));
    assert!(prepared.files_read().iter().all(|read| !read.path.starts_with("defs") || probe(&read.path)), "{:?}", prepared.files_read());
}

// --- keys merged entry by entry ----------------------------------------------------------

/// A float with the content `content` and the caption `caption`, of the type `kind`,
/// labeled so that it is numbered.
fn float(kind: &str, content: &str, caption: &str) -> String {
    format!("\\begin{{{kind}}}{content}\\caption{{{caption}}}\\label{{{kind}:x}}\\end{{{kind}}}\n\n")
}

#[test]
fn a_package_adds_an_entry_the_front_matter_replaces_one_whole_and_false_removes_one() {
    // The float types of the program are `figure` and `table`. A package adds
    // `algorithm`; the front matter replaces `figure` and removes `table`.
    let algo = "features:\n  floats:\n    types:\n      algorithm: {handlers: [any], caption_name: Alg.}\n";
    let front = "  features:\n    floats:\n      types:\n        figure: {handlers: [any], caption_name: Figure}\n        table: false\n";
    let document = |content: &str| format!("---\nflm:\n  use: [algo]\n{front}---\n{content}");
    let files = |content: &str| {
        MemoryFiles::new().document("doc/paper.flm", &document(content)).extension("defs/algo.yaml", &flm(algo))
    };
    let (built, text) = rendered(files(&(float("figure", "A.", "One") + &float("algorithm", "B.", "Two"))), request(&["defs"]));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // The entry `figure` is the front matter's, whole: the program's number format (Roman
    // numerals) is not kept.
    let text = text.replace('\u{a0}', " ");
    assert!(text.contains("Figure 1: One") && text.contains("Alg. 1: Two"), "{text}");
    let removed = build(&prepared(files(&float("table", "C.", "Three")), request(&["defs"])).unwrap(), &Scope::none())
        .unwrap();
    assert!(removed.has_errors(), "{:?}", removed.diagnostics);
    flm_core::dump::diag::expect(&removed.diagnostics, "latexlike.environments.unknown-environment")
        .data(serde_json::json!({"name": "table"}));
    // Two packages giving `algorithm` two values: one problem, for that entry.
    let peers = MemoryFiles::new()
        .document("doc/paper.flm", &paper("algo, other", ""))
        .extension("defs/algo.yaml", &flm(algo))
        .extension("defs/other.yaml", &flm(&algo.replace("Alg.", "Algorithm")));
    let found = problems(peers, request(&["defs"]), PrepareErrorKind::Settings);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("defs/algo.yaml:5:"), "{found:?}");
}

#[test]
fn an_option_naming_one_entry_sets_it_and_leaves_the_others() {
    let files = MemoryFiles::new().document("doc/paper.flm", "A\\footnote{x} B\\aside{y}.\n");
    let mut with_aside = request(&[]);
    let aside = "xnotes.kinds.aside={commands: [aside], words: {base: {one: Aside~, other: Asides~}}}";
    with_aside.options.push(CommandLineSetting::new(aside, format!("-O {aside}")));
    let (built, text) = rendered(files, with_aside);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // The program's footnote keeps its letter; the aside is numbered.
    assert!(text.starts_with("Aᵃ B¹."), "{text}");
}

#[test]
fn the_four_configurations_arrive_from_a_package_and_from_a_front_matter_as_the_rust_values() {
    use flm_core::stdlib::features::cells::CellsConfig;
    use flm_core::stdlib::features::citations::{CitationPlacement, CitationsConfig, MarkForm, default_style};
    use flm_core::stdlib::features::floats::{FloatType, FloatsConfig};
    use flm_core::stdlib::features::graphics::GraphicsConfig;
    use flm_host::seed::read_front_matter;
    use flm_host::settings::{Settings, Source, add_block};

    let features = "  graphics: {global: true}\n  cells: {global: true}\n  floats:\n    types:\n      algorithm: {handlers: [any], caption_name: Alg.}\n  citations: {placement: in-place, mark: superscript, heading_title: null, heading_level: 3, note_separator: ', '}\n";
    // Read from the block through the ranks, the values are the Rust values.
    let node = read_front_matter(&format!("features:\n{features}"), 1).unwrap();
    let block = flm_ext::block::read(&node.to_json()).unwrap();
    let source = Source::Package {
        factory: "defs/four.yaml#_".into(),
        file: "defs/four.yaml".into(),
        stem: "four".into(),
        name: "four".into(),
        package: "_".into(),
    };
    let mut settings = Settings::new();
    add_block(&mut settings, &block, &source, Some(&node));
    let parts = flm_host::stdpkg::StdParts::new(Arc::new(flm_host::output::Naming::Folder));
    let std = flm_host::stdpkg::factory(parts).unwrap();
    let read = settings.read(&[&std]).unwrap();
    let package = &read.packages[std.name()];
    assert_eq!(package.config::<GraphicsConfig>("graphics"), Some(&GraphicsConfig { global: true }));
    assert_eq!(package.config::<CellsConfig>("cells"), Some(&CellsConfig { global: true }));
    let mut floats = FloatsConfig::default();
    floats.types.push(FloatType::new("algorithm", ["any"]).caption_name("Alg."));
    assert_eq!(package.config::<FloatsConfig>("floats"), Some(&floats));
    let citations = CitationsConfig {
        placement: CitationPlacement::InPlace,
        mark: MarkForm::Superscript,
        style: default_style(),
        heading_title: None,
        heading_level: 3,
        note_separator: ", ".into(),
    };
    assert_eq!(package.config::<CitationsConfig>("citations"), Some(&citations));
    // Through the whole preparation, the package and the front matter give one registry,
    // another than the program's own. (The document uses the package `four` each time: a
    // package in use is part of the registry, whatever it sets.)
    let front = features.lines().map(|line| format!("    {line}\n")).collect::<String>();
    let from_package = MemoryFiles::new()
        .document("doc/paper.flm", &paper("four", ""))
        .extension("defs/four.yaml", &flm(&format!("features:\n{features}")));
    let from_front = MemoryFiles::new()
        .document("doc/paper.flm", &paper("four", &format!("  features:\n{front}")))
        .extension("defs/four.yaml", "extension: {description: Nothing set.}\n");
    let bare = MemoryFiles::new()
        .document("doc/paper.flm", &paper("four", ""))
        .extension("defs/four.yaml", "extension: {description: Nothing set.}\n");
    let digest = |files: MemoryFiles| prepared(files, request(&["defs"])).expect("prepared").registry().digest();
    assert_eq!(digest(from_package), digest(from_front.clone()));
    assert_ne!(digest(from_front), digest(bare));
}

// --- 11. the files a compose template ships -------------------------------------------------

/// A compose template `fancy` for HTML shipping an applied stylesheet and a logo at
/// `logo`, a path relative to the description's folder.
fn fancy(logo: &str) -> String {
    format!(
        "name: fancy\noutput: html\nengine: minijinja\nmain: fancy.html\naccepts: all\nsettings: {{type: object, properties: {{color: {{type: string, default: teal}}}}}}\nartifacts:\n  \
         style:\n    file: look.css\n    template: true\n  logo:\n    file: {logo}\n"
    )
}

/// The name of the logo of the extension `fancy`, as its template's description reaches it
/// (`../../../img/logo.png`): the folder of the description joined with that path as text,
/// the `..` as written.
const FANCY_LOGO: &str = "defs/fancy/templates/html/fancy/../../../img/logo.png";

/// The extension `fancy` of `defs/` (a folder), its template shipping a logo at `logo`, and
/// a document using it.
fn shipping(logo: &str) -> MemoryFiles {
    let mut files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("fancy", ""))
        .extension("defs/fancy/flmextension.yaml", "")
        .with("defs/fancy/templates/html/fancy/flmtemplate.yaml", &fancy(logo))
        .with(
            "defs/fancy/templates/html/fancy/fancy.html",
            "<link rel=\"stylesheet\" href=\"{{ artifacts.style }}\">{{ regions.content }}\n",
        )
        .with("defs/fancy/templates/html/fancy/look.css", "p { color: {{ config.color }}; background: url({{ artifacts.logo }}) }\n");
    files.insert("defs/fancy/img/logo.png", vec![0x89, b'P', b'N', b'G', 0xff]);
    files
}

/// A request for the workflow `html` of `doc/paper.flm` through the compose template `fancy`.
fn fancy_request() -> Request {
    let mut chosen = request(&["defs"]);
    chosen.profile = Some(String::from("html"));
    chosen.options.push(CommandLineSetting::new("compose.template='fancy'", "-t fancy"));
    chosen
}

#[test]
fn a_template_of_a_folder_extension_ships_its_files() {
    let files = shipping("../../../img/logo.png");
    let sink = Arc::new(MemorySink::new());
    let mut chosen = fancy_request();
    chosen.naming = flm_host::output::Naming::File { name: String::from("paper.html") };
    chosen.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let ready = prepared(files, chosen).expect("prepared");
    let found: Vec<String> = ready.files_read().iter().filter(|read| read.found).map(|read| read.path.clone()).collect();
    for path in ["defs/fancy/templates/html/fancy/look.css", FANCY_LOGO] {
        assert!(found.iter().any(|found| found == path), "{path}: {found:?}");
    }
    let built = build(&ready, &Scope::none()).expect("built");
    assert_only_unchecked_output(&built.diagnostics, &["html"]);
    let received = sink.files();
    assert_eq!(received.keys().collect::<Vec<_>>(), ["paper-logo.png", "paper-style.css", "paper.html"]);
    let page = String::from_utf8(received["paper.html"].bytes.clone()).unwrap();
    assert!(page.starts_with("<link rel=\"stylesheet\" href=\"paper-style.css\">"), "{page}");
    let style = String::from_utf8(received["paper-style.css"].bytes.clone()).unwrap();
    assert_eq!(style, "p { color: teal; background: url(paper-logo.png) }\n");
    assert_eq!(received["paper-style.css"].media_type, "text/css");
    assert_eq!(received["paper-logo.png"].bytes, [0x89, b'P', b'N', b'G', 0xff]);
    assert_eq!(received["paper-logo.png"].media_type, "image/png");
}

#[test]
fn a_shipped_file_leaving_the_extension_s_folder_or_missing_is_refused() {
    let leaving = problems(shipping("../../../../outside.png"), fancy_request(), PrepareErrorKind::Packages);
    assert_eq!(leaving.len(), 1, "{leaving:?}");
    let missing = problems(shipping("nosuch.png"), fancy_request(), PrepareErrorKind::Packages);
    assert_eq!(missing.len(), 1, "{missing:?}");
}

/// A sink that keeps every delivery it receives.
#[derive(Default)]
struct Deliveries(std::sync::Mutex<Vec<Vec<flm_host::output::NamedFile>>>);

impl Sink for Deliveries {
    fn receive(&self, files: &[flm_host::output::NamedFile], _progress: &Scope) -> Result<(), Vec<flm_host::output::OutputError>> {
        self.0.lock().unwrap().push(files.to_vec());
        Ok(())
    }
}

/// An extension `pair` declaring the workflow `pair` (the outputs `html` and `text`) and a
/// compose template `pair` for each output, the page's shipping `shared.css` and the text's
/// shipping `text_file`, both as the artifact `shared`; a document using it; the request.
fn pair(text_file: &str) -> (MemoryFiles, Request, Arc<Deliveries>) {
    let description = |output: &str, text: &str, file: &str| {
        format!(
            "name: pair\noutput: {output}\nengine: minijinja\nmain: {text}\naccepts: all\nartifacts:\n  shared: {{file: \
             ../../../{file}}}\n"
        )
    };
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &paper("pair", ""))
        .extension("defs/pair/flmextension.yaml", &flm("workflows:\n  pair:\n    extends: html\n    outputs: [html, text]\n"))
        .with("defs/pair/templates/html/pair/flmtemplate.yaml", &description("html", "pair.html", "shared.css"))
        .with("defs/pair/templates/html/pair/pair.html", "{{ regions.content }}")
        .with("defs/pair/templates/text/pair/flmtemplate.yaml", &description("text", "pair.txt", text_file))
        .with("defs/pair/templates/text/pair/pair.txt", "{{ regions.content }}")
        .with("defs/pair/shared.css", "p {}\n")
        .with("defs/pair/other.css", "q {}\n");
    let sink = Arc::new(Deliveries::default());
    let mut chosen = request(&["defs"]);
    chosen.profile = Some(String::from("pair"));
    chosen.options.push(CommandLineSetting::new("compose.template='pair'", "-t pair"));
    chosen.naming = flm_host::output::Naming::File { name: String::from("paper.html") };
    chosen.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    (files, chosen, sink)
}

#[test]
fn two_outputs_shipping_one_file_hand_it_once_per_output_in_one_delivery() {
    let (files, chosen, sink) = pair("shared.css");
    let built = build(&prepared(files, chosen).expect("prepared"), &Scope::none()).expect("built");
    assert_only_unchecked_output(&built.diagnostics, &["html"]);
    let deliveries = sink.0.lock().unwrap();
    assert_eq!(deliveries.len(), 1);
    let listed: Vec<(&str, &str)> = deliveries[0].iter().map(|file| (file.output.as_str(), file.path.as_str())).collect();
    assert_eq!(
        listed,
        [("html", "paper-shared.css"), ("html", "paper.html"), ("text", "paper-shared.css"), ("text", "paper.txt")]
    );
}

#[test]
fn two_outputs_writing_different_files_at_one_path_write_nothing() {
    let (files, chosen, sink) = pair("other.css");
    let built = build(&prepared(files, chosen).expect("prepared"), &Scope::none()).expect("built");
    let failed: Vec<&str> = built
        .diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.data().downcast_ref::<flm_host::stdpkg::write::WriteFailed>())
        .map(|failed| failed.output.as_str())
        .collect();
    assert_eq!(failed, ["text"], "{:?}", built.diagnostics);
    assert!(sink.0.lock().unwrap().is_empty());
}

// --- 12. the order of features ------------------------------------------------------------

#[test]
fn a_chain_of_an_extension_and_the_reverse_chain_of_the_front_matter_are_a_cycle_naming_both() {
    let document = paper("ordered", "  feature_order:\n    - toc < headings\n");
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &document)
        .extension("defs/ordered.yaml", &flm("feature_order: headings < toc"));
    let found = problems(files, request(&["defs"]), PrepareErrorKind::Registry);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("::"), "{found:?}");
}

#[test]
fn a_feature_no_package_has_is_a_warning_for_each_source_that_writes_it() {
    // The front matter and a package of an extension each write a feature that no package in
    // use has, the front matter a switch and a key: one warning for each source, at the
    // first place it writes it, and the document is built.
    let document = paper("quiet", "  features:\n    nosuch: {enabled: false, x: 1}\n");
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &document)
        .extension("defs/quiet.yaml", &flm("features:\n  nosuch: false"));
    let prepared = prepared(files, request(&["defs"])).expect("prepared");
    let warnings: Vec<String> = prepared
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.identifier() == UnknownFeature::IDENTIFIER)
        .map(|diagnostic| {
            assert_eq!(diagnostic.severity(), flm_core::techy::error::Severity::Warning);
            // Placed by its line and column, with no range of the parsed text.
            assert_eq!(Reported::of_diagnostic(diagnostic, &ContentOffsets::new()).bytes, None);
            flm_host::report::diagnostic(diagnostic)
        })
        .collect();
    // Each is laid out as the severity and the message, then the place.
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    for (printed, place) in warnings.iter().zip(["defs/quiet.yaml:3:13", "doc/paper.flm:5:23"]) {
        assert!(printed.starts_with("warning: ") && printed.ends_with(&format!("\n  at {place}")), "{printed}");
    }
    assert!(!build(&prepared, &Scope::none()).expect("built").has_errors());
    assert!(!prepared.settings_in_effect().contains_key("nosuch"));
}

/// Files in memory that a test changes between the preparation and the build.
struct Changing(Mutex<MemoryFiles>);

impl Files for Changing {
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &str) -> Result<Vec<String>, FileError> {
        self.0.lock().unwrap().entries(path)
    }
}

#[test]
fn the_warning_of_a_false_comes_with_the_problems_when_the_work_stops() {
    // `false` for a feature that no package has, then a problem that stops the
    // preparation, at the settings or at the registry, or the build: the error carries the
    // warning, at its place.
    // The warning, by its identifier, its severity and its place.
    let warning = [(
        String::from(UnknownFeature::IDENTIFIER),
        flm_core::techy::error::Severity::Warning,
        Some(Place { file: Some(String::from("doc/paper.flm")), line: Some(5), column: Some(13), ..Place::default() }),
    )];
    let carried = |error: &PrepareError| -> Vec<(String, flm_core::techy::error::Severity, Option<Place>)> {
        error
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let Record::Diagnostic { place, .. } = Record::of_diagnostic(diagnostic, &ContentOffsets::new()) else {
                    unreachable!("a diagnostic's record")
                };
                (diagnostic.identifier().to_string(), diagnostic.severity(), place)
            })
            .collect()
    };
    let stopped = |written: fn(MemoryFiles, &str, &str) -> MemoryFiles, uses: &str, more: &str, kind: PrepareErrorKind| {
        let files = written(MemoryFiles::new(), "doc/paper.flm", &paper(uses, more));
        let error = prepared(files, request(&[])).expect_err("a problem");
        assert_eq!(error.kind, kind, "{error:?}");
        error
    };
    // Beside a problem with the settings.
    let error =
        stopped(Written::refused_document, "std", "  features:\n    nosuch: false\n    text: {wdth: 3}\n", PrepareErrorKind::Settings);
    assert_eq!(carried(&error), warning);
    assert_eq!(error.problems.len(), 1, "{:?}", error.problems);
    // Beside a problem that stops the registry build: chains in a circle.
    let error = stopped(
        Written::faulty,
        "std",
        "  features:\n    nosuch: false\n  feature_order: [headings < toc, toc < headings]\n",
        PrepareErrorKind::Registry,
    );
    assert_eq!(carried(&error), warning);
    // Before the settings are read, there is no warning to carry.
    let error = stopped(Written::faulty, "nosuchpackage", "  features:\n    nosuch: false\n", PrepareErrorKind::Packages);
    assert!(error.diagnostics.is_empty());
    // A build that stops carries the preparation's diagnostics.
    let document = paper("std", "  features:\n    nosuch: false\n");
    let files = Arc::new(Changing(Mutex::new(MemoryFiles::new().document("doc/paper.flm", &document))));
    let prepared = prepare(request(&[]), Arc::clone(&files) as Arc<dyn Files>, &[], &Scope::none()).expect("prepared");
    *files.0.lock().unwrap() = MemoryFiles::new().faulty("doc/paper.flm", "---\nflm: {}\n");
    let error = build(&prepared, &Scope::none()).err().expect("a problem");
    assert_eq!(error.kind, PrepareErrorKind::FrontMatter, "{error:?}");
    assert_eq!(carried(&error), warning);
}

#[test]
fn the_chains_of_an_extension_and_of_the_front_matter_that_agree_are_read() {
    let document = paper("ordered", "  feature_order: [headings < toc, nosuchfeature < toc]\n");
    let files = MemoryFiles::new()
        .document("doc/paper.flm", &document)
        .extension("defs/ordered.yaml", &flm("feature_order: headings < toc"));
    assert!(widest(files, request(&["defs"])) > 0);
}
