//! The lecture-notes extension (`extensions/data/lecturenotes/`) rendering documents end
//! to end: the sample document of `tests/documents/lecturenotes/` and small documents,
//! each copied into a folder of its own under the build folder and rendered with
//! `--extensions-dir` naming the repository's `extensions/data`; then the extension's
//! description checked against the build of its bundle (`npm/lecturenotes`), and the
//! committed bundle checked against a fresh build.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository's root folder.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The repository's folder `extensions/data`.
fn extensions_data() -> PathBuf {
    repository().join("extensions/data")
}

/// The extension's folder.
fn extension() -> PathBuf {
    extensions_data().join("lecturenotes")
}

/// The folder of the committed bundle: `dist/` in the compose template's folder.
fn dist_folder() -> PathBuf {
    extension().join("templates/html/lecturenotes/dist")
}

/// The committed bundle's file `NAME`, as text.
fn dist(name: &str) -> String {
    std::fs::read_to_string(dist_folder().join(name)).unwrap()
}

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-lecturenotes-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// A folder holding a copy of the sample document and its image.
fn sample(name: &str) -> PathBuf {
    let dir = folder(name);
    let documents = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/lecturenotes");
    for file in ["notes.flm", "cycle.svg"] {
        std::fs::copy(documents.join(file), dir.join(file)).unwrap();
    }
    dir
}

/// Runs `flm render --extensions-dir <extensions/data>` with `args` in the folder `cwd`, its
/// home folder there too.
fn flm(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("render")
        .arg("--extensions-dir")
        .arg(extensions_data())
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The settings of the description, as the glue script writes them when a document gives
/// none, before the settings the sample gives.
const FONT_FAMILY: &str =
    r#"Georgia, "Iowan Old Style", Charter, "Bitstream Charter", "Palatino Linotype", Palatino, "Book Antiqua", "Times New Roman", serif"#;

#[test]
fn the_sample_renders_into_the_page_and_its_three_shipped_files() {
    let dir = sample("sample");
    let output = flm(&dir, &["-o", "out/notes.html", "notes.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let out = dir.join("out");
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    assert_eq!(names, ["notes-fragment.html", "notes-script.js", "notes-style.css", "notes.html"]);

    // The page: the glue stylesheet, then one link per entry of `stylesheets`; the bar with
    // the `links`; the sidebar with the table of contents; the content region; the glue
    // script after the library's scripts.
    let page = read(out.join("notes.html"));
    assert!(page.starts_with("<!DOCTYPE html>\n<html>\n"), "{page}");
    assert!(
        page.contains(
            "    <link rel=\"stylesheet\" href=\"notes-style.css\">\n    <link rel=\"stylesheet\" href=\"notes-extra.css\">\n  </head>"
        ),
        "{page}"
    );
    assert!(page.contains(
        "        <span class=\"ln-topbar-links\">\n\
         \x20         <a class=\"ln-topbar-link\" href=\"https:&#x2f;&#x2f;example.org&#x2f;course&#x2f;\">Course page</a>\n\
         \x20         <a class=\"ln-topbar-link\" href=\"exercises.html\">Exercises</a>\n\
         \x20       </span>\n      </header>"
    ));
    let sidebar = &page[page.find("<aside class=\"ln-sidebar\">").unwrap()..page.find("</aside>").unwrap()];
    assert!(sidebar.contains("<button class=\"ln-toc-toggle\""));
    assert!(sidebar.contains("class=\"flm-pres-block flm-toc\""), "{sidebar}");
    assert!(page.contains("<header class=\"ln-titleblock\">"));
    assert!(page.contains("<article class=\"flm-content\" id=\"flm-content\">"));
    assert!(page.contains("<section class=\"ln-backmatter\">"));
    assert!(page.ends_with("    <script src=\"notes-script.js\"></script>\n  </body>\n</html>\n"), "{page}");
    for absent in ["data-sidebar", "data-toc", "data-chapters", "ln-config", "ln-proto"] {
        assert!(!page.contains(absent), "{absent}");
    }

    // The glue stylesheet: the font family, then the bundle's stylesheet as it is.
    assert_eq!(
        read(out.join("notes-style.css")),
        format!(":root {{ --ln-family: {FONT_FAMILY}; }}\n{}\n", dist("lecturenotes.css"))
    );
    // The glue script: the bundle's script as it is, then the settings as JSON.
    let settings = serde_json::json!({
        "chapter_level": 1,
        "font_family": FONT_FAMILY,
        "stylesheets": ["notes-extra.css"],
        "links": [
            {"text": "Course page", "href": "https://example.org/course/"},
            {"text": "Exercises", "href": "exercises.html"},
        ],
    });
    let script = read(out.join("notes-script.js"));
    let bundle = dist("lecturenotes.js");
    assert!(script.starts_with(&bundle));
    let call = script[bundle.len()..].trim();
    let json = call.strip_prefix("window.flmLecturenotes.setup(").and_then(|rest| rest.strip_suffix(");")).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(json).unwrap(), settings);

    // The fragment: the title block, the table of contents and the content region, no
    // page around them.
    let fragment = read(out.join("notes-fragment.html"));
    assert!(fragment.contains("<header class=\"ln-titleblock\">"));
    assert!(fragment.contains("class=\"flm-pres-block flm-toc\""));
    assert!(fragment.contains("<article class=\"flm-content\" id=\"flm-content\">"));
    assert!(fragment.contains("<section class=\"ln-backmatter\">"));
    for absent in ["<html", "<head>", "<body", "ln-topbar"] {
        assert!(!fragment.contains(absent), "{absent}");
    }
    let content = |text: &str| {
        let start = text.find("<article class=\"flm-content\" id=\"flm-content\">").unwrap();
        text[start..start + text[start..].find("</article>").unwrap()].trim_end().to_string()
    };
    assert_eq!(content(&fragment), content(&page));
    let styles = fragment.find("<!-- The library's styles -->").unwrap();
    let scripts = fragment.find("<!-- The library's scripts -->").unwrap();
    assert!(styles < fragment.find("<article").unwrap() && fragment.rfind("</section>").unwrap() < scripts);
}

#[test]
fn a_document_s_settings_reach_the_glue_files_and_the_page() {
    let dir = folder("settings");
    std::fs::write(
        dir.join("paper.flm"),
        "---\nflm:\n  use: [lecturenotes]\n  features:\n    compose:\n      template_config:\n        html:\n          lecturenotes:\n            \
         chapter_level: 2\n            font_family: '\"Source Serif 4\", serif'\n---\n\\section{One}\n\nText.\n",
    )
    .unwrap();
    let output = flm(&dir, &["-o", "out/paper.html", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let out = dir.join("out");
    assert!(read(out.join("paper-style.css")).starts_with(":root { --ln-family: \"Source Serif 4\", serif; }\n"));
    let script = read(out.join("paper-script.js"));
    assert!(script.ends_with(
        "\nwindow.flmLecturenotes.setup({\"chapter_level\":2,\"font_family\":\"\\\"Source Serif 4\\\", serif\",\"stylesheets\":[],\"links\":[]});\n"
    ));
    // With no `links`, no links in the bar; with no `stylesheets`, the glue stylesheet alone.
    let page = read(out.join("paper.html"));
    assert!(!page.contains("ln-topbar-links"));
    assert!(page.contains("    <link rel=\"stylesheet\" href=\"paper-style.css\">\n  </head>"));
}

#[test]
fn a_document_using_the_extension_renders_as_text() {
    // The extension names its compose template for the output `html` only; the output
    // `text` keeps its own default. The browser check's document, `npm/lecturenotes/check/plain.flm`.
    let dir = folder("text");
    std::fs::copy(repository().join("npm/lecturenotes/check/plain.flm"), dir.join("plain.flm")).unwrap();
    let output = flm(&dir, &["-p", "text", "-o", "out/plain.txt", "plain.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let text = read(dir.join("out/plain.txt"));
    assert!(text.contains("A short document") && text.contains("Another paragraph."), "{text}");
    assert!(!text.contains("ln-"), "{text}");
}

#[test]
fn a_document_with_parts_needs_the_root_heading_level_zero() {
    let document = |root: &str| {
        format!(
            "---\nflm:\n  use: [lecturenotes]\n  features:\n{root}    headings:\n      headings:\n        \
             part: {{level: 0, counter: null, style: null}}\n      commands:\n        part: {{heading: part}}\n---\n\
             \\part{{One}}\n\n\\section{{Two}}\n\nText.\n"
        )
    };
    let dir = folder("parts");
    std::fs::write(dir.join("with.flm"), document("    html:\n      root_heading_level: 0\n")).unwrap();
    std::fs::write(dir.join("without.flm"), document("")).unwrap();
    let output = flm(&dir, &["-o", "out/with.html", "with.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let output = flm(&dir, &["-o", "out/without.html", "without.flm"]);
    assert_ne!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "error: a heading's level must be at least the root heading level 1 (the render option \
             `root_heading_level`); this heading's level is 0\n  at without.flm:11:1"
        ),
        "{stderr}"
    );
}

/// The strings between double quotes on `line`, in order.
fn quoted(line: &str) -> Vec<String> {
    line.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

#[test]
fn the_description_s_delimiters_are_those_the_bundle_s_build_checks() {
    let description = read(extension().join("templates/html/lecturenotes/flmtemplate.yaml"));
    let mut lines = description.lines().skip_while(|line| *line != "delimiters:").skip(1);
    let mut given = Vec::new();
    for key in ["block", "variable", "comment"] {
        let line = lines.next().unwrap().trim();
        let value = line.strip_prefix(key).and_then(|rest| rest.strip_prefix(": ")).unwrap_or_else(|| panic!("{line}"));
        given.extend(quoted(value));
    }
    let build = read(repository().join("npm/lecturenotes/build.mjs"));
    let constant = build.lines().find(|line| line.starts_with("const DELIMITERS = ")).unwrap();
    let checked: Vec<String> =
        constant.split('\'').skip(1).step_by(2).map(str::to_string).collect();
    assert_eq!(given, checked);
    assert_eq!(given.len(), 6);
}

#[test]
fn the_committed_bundle_matches_a_fresh_build() {
    let workspace = repository().join("npm");
    let member = workspace.join("lecturenotes");
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: `node` is not on the PATH, so the bundle built by {} cannot be rebuilt", member.display());
        return;
    }
    if !workspace.join("node_modules/esbuild").is_dir() {
        eprintln!("skipped: the npm workspace's dependencies are not installed (run `npm install` in {})", workspace.display());
        return;
    }
    let out = folder("dist");
    let output = Command::new("node").arg("build.mjs").arg("--out").arg(&out).current_dir(&member).output().unwrap();
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    for name in ["lecturenotes.js", "lecturenotes.css"] {
        let fresh = std::fs::read(out.join(name)).unwrap();
        let committed = std::fs::read(dist_folder().join(name)).unwrap();
        assert!(
            fresh == committed,
            "`{}` differs from a fresh build: run `npm run build` in {}",
            dist_folder().join(name).display(),
            member.display()
        );
    }
}
