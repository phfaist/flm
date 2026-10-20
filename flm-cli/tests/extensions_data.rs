//! Extensions of the data engine found by the binary: through `--extensions-dir`, through
//! the environment variable `FLMPATH`, in the folder `_flmextensions` beside the document
//! and in the user's folder; each run over files under the build folder, with a home
//! folder of its own.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-extensions-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Writes `text` at `path` below `dir`, making the folders.
fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Runs `flm render` with `args` in the folder `cwd`, its home folder there too, and
/// `FLMPATH` set to `flmpath` when given.
fn flm(cwd: &Path, args: &[&str], flmpath: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
    command.arg("render").args(args).current_dir(cwd).env("HOME", cwd).env_remove("FLMPATH").env_remove("USERPROFILE");
    if let Some(flmpath) = flmpath {
        command.env("FLMPATH", flmpath);
    }
    command.output().unwrap()
}

/// A paragraph of short words, which the text output wraps at the width set.
const PAPER: &str = "---\nflm:\n  use: [narrow]\n---\none two three four five six seven eight nine ten eleven twelve\n";

/// An extension setting the text output's width.
const NARROW: &str = "flm:\n  features:\n    text:\n      width: 20\n";

fn wrapped_at_20(output: &Output) -> bool {
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    output.status.success() && text.lines().count() > 1 && text.lines().all(|line| line.chars().count() <= 20)
}

#[test]
fn an_extension_is_found_in_a_folder_named_with_extensions_dir() {
    let dir = folder("named");
    write(&dir, "doc/paper.flm", PAPER);
    write(&dir, "defs/narrow.yaml", NARROW);
    let output = flm(&dir, &["-p", "text", "--extensions-dir", "defs", "doc/paper.flm"], None);
    assert!(wrapped_at_20(&output), "{output:?}");
}

#[test]
fn a_folder_named_on_the_command_line_must_exist() {
    let dir = folder("named-missing");
    write(&dir, "paper.flm", "Hello.\n");
    let output = flm(&dir, &["-p", "text", "--extensions-dir", "nosuch", "paper.flm"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("nosuch"));
}

#[test]
fn an_extension_is_found_through_flmpath() {
    let dir = folder("flmpath");
    write(&dir, "paper.flm", PAPER);
    write(&dir, "shared/narrow/flmextension.yaml", NARROW);
    let output = flm(&dir, &["-p", "text", "paper.flm"], Some(&dir.join("shared")));
    assert!(wrapped_at_20(&output), "{output:?}");
}

#[test]
fn an_extension_is_found_in_flmextensions_beside_the_document_and_in_the_user_s_folder() {
    let dir = folder("beside");
    write(&dir, "doc/paper.flm", PAPER);
    write(&dir, "doc/_flmextensions/narrow.yaml", NARROW);
    assert!(wrapped_at_20(&flm(&dir, &["-p", "text", "doc/paper.flm"], None)));
    let dir = folder("user");
    write(&dir, "paper.flm", PAPER);
    write(&dir, ".flm/extensions/narrow.yaml", NARROW);
    assert!(wrapped_at_20(&flm(&dir, &["-p", "text", "paper.flm"], None)));
}

/// A folder written `./_flmextensions` (for the document `./paper.flm`) or `./defs` (with
/// the option): the extension `boxed`, reached by name from the document and from the
/// extension `other`, is one package, so its template is shipped once.
#[test]
fn an_extension_reached_twice_through_a_folder_written_with_dot_is_read_once() {
    for (defs, args) in [
        ("_flmextensions", &["-p", "text", "-t", "boxed", "./paper.flm"][..]),
        ("defs", &["-p", "text", "-t", "boxed", "--extensions-dir", "./defs", "./paper.flm"][..]),
    ] {
        let dir = folder(&format!("dot-{defs}"));
        write(&dir, "paper.flm", "---\nflm:\n  use: [boxed, other]\n---\nHello.\n");
        write(&dir, &format!("{defs}/other.yaml"), "flm:\n  use: [boxed]\n");
        write(&dir, &format!("{defs}/boxed/flmextension.yaml"), "extension:\n  description: A boxed page.\n");
        write(
            &dir,
            &format!("{defs}/boxed/templates/text/boxed/flmtemplate.yaml"),
            "output: text\nengine: minijinja\nmain: boxed.txt\naccepts: all\n",
        );
        write(&dir, &format!("{defs}/boxed/templates/text/boxed/boxed.txt"), "BEGIN\n{{ regions.content }}\nEND\n");
        let output = flm(&dir, args, None);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success() && text.starts_with("BEGIN\nHello."), "{output:?}");
    }
}

#[test]
fn an_extension_wins_over_the_extension_it_uses_and_two_that_do_not_use_each_other_must_agree() {
    let paper = PAPER.replace("[narrow]", "[article, my-article]");
    let dir = folder("my-article");
    write(&dir, "paper.flm", &paper);
    write(&dir, "_flmextensions/article.yaml", "flm:\n  features:\n    text:\n      width: 40\n");
    write(&dir, "_flmextensions/my-article.yaml", &NARROW.replace("flm:\n", "flm:\n  use: [article]\n"));
    let output = flm(&dir, &["-p", "text", "paper.flm"], None);
    assert!(wrapped_at_20(&output), "{output:?}");
    let dir = folder("my-article-peers");
    write(&dir, "paper.flm", &paper);
    write(&dir, "_flmextensions/article.yaml", "flm:\n  features:\n    text:\n      width: 40\n");
    write(&dir, "_flmextensions/my-article.yaml", NARROW);
    let output = flm(&dir, &["-p", "text", "paper.flm"], None);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("_flmextensions/article.yaml") && stderr.contains("_flmextensions/my-article.yaml"), "{stderr}");
}

/// A template text of a folder extension (the file its description's `main` gives) that is a
/// symbolic link to a file outside the extension's folder is refused: the build stops with
/// status 2 and names the link.
#[cfg(unix)]
#[test]
fn an_extension_file_linked_outside_its_folder_is_refused() {
    let dir = folder("link");
    write(&dir, "paper.flm", "---\nflm:\n  use: [boxed]\n---\nHello.\n");
    write(&dir, "_flmextensions/boxed/flmextension.yaml", "");
    write(
        &dir,
        "_flmextensions/boxed/templates/text/boxed/flmtemplate.yaml",
        "output: text\nengine: minijinja\nmain: boxed.txt\naccepts: all\n",
    );
    write(&dir, "outside.txt", "BEGIN\n{{ regions.content }}\nEND\n");
    std::os::unix::fs::symlink(dir.join("outside.txt"), dir.join("_flmextensions/boxed/templates/text/boxed/boxed.txt"))
        .unwrap();
    let output = flm(&dir, &["-p", "text", "-t", "boxed", "paper.flm"], None);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "the file `_flmextensions/boxed/templates/text/boxed/boxed.txt` of the extension `boxed` leads outside \
             the extension's folder `_flmextensions/boxed` through a symbolic link"
        ),
        "{stderr}"
    );
}

// --- case 2: an extension of the engine `data` doing all a data package did -----------------------

/// `tests/documents/extensions/data.flm` uses `house`, an extension of the engine `data`
/// written as a folder: it uses the extension `margins` (the text's width), defines a
/// macro, sets `dollar_math`, chooses the compose template it ships and defines the
/// workflow `draft`. Rendered by the default text workflow and by `draft`, as goldens.
#[test]
fn case_two_a_data_extension_uses_another_sets_features_and_parsing_ships_a_template_and_defines_a_workflow() {
    let documents = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/extensions");
    let dir = folder("case-two");
    write(&dir, "data.flm", &std::fs::read_to_string(documents.join("data.flm")).unwrap());
    for file in [
        "_flmextensions/margins.yaml",
        "_flmextensions/house/flmextension.yaml",
        "_flmextensions/house/templates/text/house/flmtemplate.yaml",
        "_flmextensions/house/templates/text/house/house.txt",
    ] {
        write(&dir, file, &std::fs::read_to_string(documents.join(file)).unwrap());
    }
    let golden = |name: &str| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/extensions").join(name);
    for (args, name, width) in [(&["-p", "text"][..], "data.txt", 50), (&["-p", "draft"][..], "data-draft.txt", 30)] {
        let output = flm(&dir, &[args, &["data.flm"]].concat(), None);
        assert!(output.status.success() && output.stderr.is_empty(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        // The template, the macro, the mathematics delimited by `$`, the width.
        assert!(text.starts_with("== HOUSE ==\n") && text.contains("the Example project") && text.contains("\\(x\\)"), "{text}");
        assert!(text.lines().all(|line| line.chars().count() <= width), "{text}");
        flm_core::dump::golden::assert_text_golden(golden(name), &text);
    }
}
