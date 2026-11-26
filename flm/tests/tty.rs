//! The extension `profile-tty`, compiled into the binary (cargo feature
//! `bundled-extensions`): its workflow `tty`, chosen with `-p tty`, paints the text output
//! with the escape sequences of a terminal.
//!
//! The document holds a title, a section heading, two deeper headings, emphasized, bold
//! and monospaced spans, inline verbatim, a verbatim block holding a control character, an
//! external link long enough to wrap inside a list item, a table with a bold cell, a
//! reference inside the document and a footnote. Rendered with `-p tty -o -`, it equals the
//! golden `tests/goldens/tty/document.ansi` byte for byte (`UPDATE_GOLDEN=1` rewrites it).
//! With its escape sequences removed, it equals the output of `-p text -O
//! text.table_frame=box -O text.heading_rules=unicode -o -` (the settings of the workflow
//! `tty`) with each control character written as U+FFFD, at the default width and at `-O
//! text.width=40`, and under each value of `tty.colors`. The default output colors the
//! heading lines and the table's frame, and paints the bold cell bold; `-O tty.colors=none`
//! writes no color; `-O tty.colors=rich` gives the bytes of `standard`, since no style has
//! a color given by its values; `-O text.table_frame=none` gives the plain table; a value of
//! `tty.colors` that is none of the three is an error, and the document is still painted,
//! without colors. Every line closes what it opens; `-o out/` writes `out/index.ansi`; and
//! a document that switches the feature `text` off cannot use the workflow `tty`, which
//! extends the workflow `text`.
//!
//! The images, from the picture `tests/documents/figures/disk.png` (48 by 32 pixels at 96
//! dots per inch): shown as a block in a figure, it prints its label and a grid of
//! half-block glyphs of 6 columns by 2 lines; `width=1in` gives 12 columns by 4 lines and
//! `width=1in, height=0.5in` 12 by 3; an inline image prints its label alone, as does a
//! block image under `-p text` and under `-p tty -O graphics.strategy.text=reference`; a
//! cell size that is not a positive number is an error, and the default applies; an SVG
//! image without a `magick` program prints its label, with the publish stage's error and
//! `tty.graphics-not-shown`. A gated test (`FLM_TESTS_MAGICK=1`) draws an SVG image through
//! ImageMagick at its stated size; otherwise it writes one line beginning `skipped:`.
//!
//! The drawing's colors: by default the drawing is colored with standard colors, under
//! `tty.colors=rich` with colors given by their values, and not at all under `none`, the
//! characters being the same; two pictures on one page (`figures/disk.png` and the plain
//! pale yellow `figures/pale.png`) are each colored from their own content; with its escape
//! sequences removed, the output is the output `text` under the settings of the workflow
//! `tty` and the strategy `content`, with the drawing's rows added under the label; and a
//! verbatim block an author wrote is never colored, even when it holds a drawing's glyphs.
//!
//! One test is gated: with `FLM_TESTS_WASM_GUEST=1` (and the target `wasm32-unknown-unknown`
//! installed), the script `extensions/rust/build-wasm.sh` builds the extension's WebAssembly
//! form, and the document, and a colored drawing, rendered with `--extensions-dir` pointing
//! at it, give the same bytes as the extension compiled in. Otherwise it writes one line beginning `skipped:`.
#![cfg(feature = "bundled-extensions")]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::diagnostics;
use flm_core::dump::golden::assert_text_golden;
use serde_json::Value;

/// The document of the tests: every kind of style mark, a control character in a verbatim
/// block, a link that wraps inside a list item, a table with a bold cell, a reference, a
/// title and a footnote.
const PAPER: &str = "\
---
title: The \\emph{Terminal} Paper
---
\\section{Introduction}\\label{sec:intro}

Some \\emph{emphasized} and \\textbf{bold} text, with \\texttt{code} and \\verbcode+a{b+, and a \
reference to \\ref{sec:details}.\\footnote{A \\textbf{strong} note.}

\\begin{itemize}
\\item An item with a link to \\href{https://example.com/a b}{the site of the example, long \
enough to wrap across a line} inside it.
\\end{itemize}

\\begin{table}
\\begin{cells}
\\celldata<H>{Name & Count}
\\cell{\\textbf{alpha}}\\cell<r>{1}\\\\
\\celldata<l, r>{beta & 22}
\\end{cells}
\\caption{Counts}
\\end{table}

\\subsection{Details}\\label{sec:details}

\\subsubsection{Fine points}

\\begin{verbatimtext}
a bell \u{7} rings
\\end{verbatimtext}
";

/// The settings of the text backend that the workflow `tty` gives, as options of `-p text`.
const TTY_SETTINGS: &[&str] = &["-O", "text.table_frame=box", "-O", "text.heading_rules=unicode"];

/// A fresh folder of the test's own, `NAME` under the cargo target's folder for tests,
/// holding the document `document` as `paper.flm`.
fn folder(name: &str, document: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-tty").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("paper.flm"), document).unwrap();
    dir
}

/// The path of the golden `name`.
fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/tty").join(name)
}

/// Runs `flm render paper.flm` with `args` in `dir`, with no extension installed on the
/// search path (`HOME` is the folder, `FLMPATH` is unset).
fn render(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("render")
        .arg("paper.flm")
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

/// The standard output of a render that succeeded with nothing reported.
fn rendered(dir: &Path, args: &[&str]) -> String {
    let output = render(dir, args);
    assert!(output.status.success() && output.stderr.is_empty(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// One escape sequence of the painted text: an SGR sequence with its parameters, or an
/// OSC 8 hyperlink sequence with its target (empty for the one that closes a hyperlink).
#[derive(Debug, PartialEq, Eq)]
enum Sequence {
    Sgr(String),
    Link(String),
}

/// Splits `painted` into the text it shows and its escape sequences, in order. An escape
/// character that does not start one of the two kinds of sequence the painter writes is a
/// test failure.
fn scan(painted: &str) -> (String, Vec<Sequence>) {
    let mut shown = String::new();
    let mut sequences = Vec::new();
    let mut rest = painted;
    while let Some(at) = rest.find('\u{1b}') {
        shown.push_str(&rest[..at]);
        let sequence = &rest[at..];
        if let Some(after) = sequence.strip_prefix("\u{1b}[") {
            let end = after.find('m').unwrap_or_else(|| panic!("an SGR sequence ends: {sequence:?}"));
            let parameters = &after[..end];
            assert!(parameters.chars().all(|c| c.is_ascii_digit() || c == ';'), "{parameters:?}");
            sequences.push(Sequence::Sgr(String::from(parameters)));
            rest = &after[end + 1..];
        } else if let Some(after) = sequence.strip_prefix("\u{1b}]8;;") {
            let end = after.find("\u{1b}\\").unwrap_or_else(|| panic!("an OSC 8 sequence ends: {sequence:?}"));
            sequences.push(Sequence::Link(String::from(&after[..end])));
            rest = &after[end + 2..];
        } else {
            panic!("an escape character the painter does not write: {sequence:?}");
        }
    }
    shown.push_str(rest);
    (shown, sequences)
}

/// The text `painted` shows, its escape sequences removed.
fn shown(painted: &str) -> String {
    scan(painted).0
}

/// `text` with each control character other than the newline and the tab written as
/// U+FFFD, as the painter writes it.
fn replaced(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' | '\t' => c,
            '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' => '\u{fffd}',
            _ => c,
        })
        .collect()
}

/// Checks that each line of `painted` closes what it opens: read from its start with no
/// attribute set and no hyperlink open, it ends with none set and none open.
fn assert_every_line_closes_what_it_opens(painted: &str) {
    for line in painted.split('\n') {
        let mut styled = false;
        let mut linked = false;
        for sequence in scan(line).1 {
            match sequence {
                Sequence::Sgr(parameters) => styled = parameters != "0",
                Sequence::Link(target) => linked = !target.is_empty(),
            }
        }
        assert!(!styled && !linked, "a line leaves a style or a hyperlink open: {line:?}");
    }
}

#[test]
fn the_document_paints_as_the_golden_shows() {
    let dir = folder("golden", PAPER);
    let painted = rendered(&dir, &["-p", "tty", "-o", "-"]);
    assert_text_golden(golden("document.ansi"), &painted);
    assert_every_line_closes_what_it_opens(&painted);
}

#[test]
fn the_painted_text_shows_the_text_output_with_control_characters_replaced() {
    let dir = folder("shown", PAPER);
    let painted = rendered(&dir, &["-p", "tty", "-o", "-"]);
    let text = rendered(&dir, &[&["-p", "text"], TTY_SETTINGS, &["-o", "-"]].concat());
    assert!(text.contains('\u{7}') && text.contains('┌') && text.contains('═'), "{text:?}");
    assert_eq!(shown(&painted), replaced(&text));
    for colors in ["tty.colors=none", "tty.colors=standard", "tty.colors=rich"] {
        assert_eq!(shown(&rendered(&dir, &["-p", "tty", "-O", colors, "-o", "-"])), replaced(&text), "{colors}");
    }

    // The text backend's width applies to both outputs alike.
    let narrow_painted = rendered(&dir, &["-p", "tty", "-O", "text.width=40", "-o", "-"]);
    let narrow_text = rendered(&dir, &[&["-p", "text", "-O", "text.width=40"], TTY_SETTINGS, &["-o", "-"]].concat());
    assert_ne!(narrow_text, text);
    assert_eq!(shown(&narrow_painted), replaced(&narrow_text));
    assert!(shown(&narrow_painted).lines().all(|line| line.chars().count() <= 40), "{narrow_painted:?}");
    assert_every_line_closes_what_it_opens(&narrow_painted);
}

#[test]
fn an_output_folder_receives_index_ansi() {
    let dir = folder("folder", PAPER);
    let output = render(&dir, &["-p", "tty", "-o", "out/"]);
    assert!(output.status.success() && output.stderr.is_empty(), "{output:?}");
    // The program's record of the files it wrote begins with a dot.
    let mut written: Vec<String> = std::fs::read_dir(dir.join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !name.starts_with('.'))
        .collect();
    written.sort();
    assert_eq!(written, ["index.ansi"]);
    let painted = rendered(&dir, &["-p", "tty", "-o", "-"]);
    assert_eq!(std::fs::read_to_string(dir.join("out/index.ansi")).unwrap(), painted);
}

/// The workflow `tty` extends the workflow `text`: with the feature `text` switched off,
/// the workflow `text` does not exist, and the registration of the extension's package
/// fails.
#[test]
fn a_document_that_switches_text_off_cannot_use_the_workflow_tty() {
    let dir = folder("text-off", "---\nflm:\n  features:\n    text: false\n---\nHello.\n");
    let output = render(&dir, &["-p", "tty", "-o", "-"]);
    let said = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(output.stdout.is_empty(), "{output:?}");
}

/// The SGR parameters of the sequences on the line of `painted` that shows `needle`.
fn sgr_on_the_line_of(painted: &str, needle: &str) -> Vec<String> {
    let line = painted.lines().find(|line| shown(line).contains(needle)).unwrap_or_else(|| panic!("{needle:?}"));
    sgr_parameters(line)
}

/// The SGR parameters of the sequences on the line of `painted` that shows `shows` alone.
fn sgr_on_the_line(painted: &str, shows: &str) -> Vec<String> {
    let line = painted.lines().find(|line| shown(line) == shows).unwrap_or_else(|| panic!("{shows:?}"));
    sgr_parameters(line)
}

/// The parameters of the SGR sequences of `painted`, in order.
fn sgr_parameters(painted: &str) -> Vec<String> {
    scan(painted)
        .1
        .into_iter()
        .filter_map(|sequence| match sequence {
            Sequence::Sgr(parameters) => Some(parameters),
            Sequence::Link(_) => None,
        })
        .collect()
}

/// The parameters of the last SGR sequence written before the first `needle` that `painted`
/// holds.
fn sgr_before(painted: &str, needle: &str) -> String {
    let at = painted.find(needle).unwrap_or_else(|| panic!("{needle:?}"));
    let before = &painted[..at];
    let start = before.rfind("\u{1b}[").unwrap_or_else(|| panic!("an SGR sequence before {needle:?}"));
    String::from(&before[start + 2..before.len() - 1])
}

#[test]
fn the_scanner_reads_color_parameters() {
    let (text, sequences) = scan("\u{1b}[0;1;96mA\u{1b}[0;38;2;1;2;3;48;2;4;5;6mb\u{1b}[0m\n");
    assert_eq!(text, "Ab\n");
    let parameters = ["0;1;96", "0;38;2;1;2;3;48;2;4;5;6", "0"];
    assert_eq!(sequences, parameters.map(|p| Sequence::Sgr(String::from(p))));
}

/// The default output colors each heading line and the frame of the table, and paints the
/// bold cell bold.
#[test]
fn the_default_output_colors_the_headings_and_the_frame_and_paints_a_bold_cell() {
    let dir = folder("colors", PAPER);
    let painted = rendered(&dir, &["-p", "tty", "-o", "-"]);
    // The title and the section at depth 0, bold and bright cyan; the subsection at depth 1,
    // bold and cyan; the rule under the section, cyan.
    assert_eq!(sgr_on_the_line(&painted, "Introduction"), ["0;1;96", "0"]);
    assert_eq!(sgr_on_the_line(&painted, "Details"), ["0;1;36", "0"]);
    assert_eq!(sgr_on_the_line_of(&painted, "═══"), ["0;36", "0"]);
    // The frame in bright black, each bar of a row on its own.
    for frame in ["┌", "├", "└"] {
        assert_eq!(sgr_on_the_line_of(&painted, frame), ["0;90", "0"], "{frame}");
    }
    let row = sgr_on_the_line_of(&painted, "alpha");
    assert_eq!(row.iter().filter(|p| *p == "0;90").count(), 3, "{row:?}");
    assert_eq!(sgr_before(&painted, "alpha"), "0;1");
    assert_every_line_closes_what_it_opens(&painted);
}

/// Under `tty.colors=none`, no sequence sets a color, and every attribute and hyperlink
/// stays; under `rich`, the bytes are those of `standard`.
#[test]
fn colors_none_writes_no_color_and_rich_gives_the_bytes_of_standard() {
    let dir = folder("colors-none", PAPER);
    let standard = rendered(&dir, &["-p", "tty", "-o", "-"]);
    assert_eq!(rendered(&dir, &["-p", "tty", "-O", "tty.colors=standard", "-o", "-"]), standard);
    assert_eq!(rendered(&dir, &["-p", "tty", "-O", "tty.colors=rich", "-o", "-"]), standard);
    let plain = rendered(&dir, &["-p", "tty", "-O", "tty.colors=none", "-o", "-"]);
    let parameters = sgr_parameters(&plain);
    for parameters in &parameters {
        assert!(parameters.split(';').all(|p| ["0", "1", "3", "4"].contains(&p)), "{parameters}");
    }
    assert!(parameters.iter().any(|p| p == "0;1") && parameters.iter().any(|p| p == "0;3"), "{parameters:?}");
    assert!(scan(&plain).1.contains(&Sequence::Link(String::from("https://example.com/a%20b"))));
    assert_eq!(sgr_before(&plain, "alpha"), "0;1");
    assert_every_line_closes_what_it_opens(&plain);
}

/// `-O text.table_frame=none` under `-p tty` gives the table of `-p text`, its columns two
/// spaces apart, and keeps the workflow's other setting.
#[test]
fn table_frame_none_gives_the_plain_table_under_tty() {
    let dir = folder("frame-none", PAPER);
    let painted = rendered(&dir, &["-p", "tty", "-O", "text.table_frame=none", "-o", "-"]);
    let text = rendered(&dir, &["-p", "text", "-O", "text.heading_rules=unicode", "-o", "-"]);
    assert_eq!(shown(&painted), replaced(&text));
    assert!(!text.contains('│') && text.contains('═'), "{text}");
    // The rule under the header row is the table's border.
    assert_eq!(sgr_on_the_line_of(&painted, "-----"), ["0;90", "0"]);
}

/// A value of `tty.colors` that is none of the three is an error of the stage; the document
/// is still painted, without colors, which `--continue-with-errors` writes.
#[test]
fn an_invalid_colors_value_is_an_error_and_the_document_is_still_painted() {
    let dir = folder("colors-invalid", PAPER);
    let output = render(&dir, &["-p", "tty", "-O", "tty.colors=blue", "--continue-with-errors", "--json-messages", "-o", "-"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(diagnostics(&output).0, [common::reported("error", "tty.invalid-colors")], "{output:?}");
    let painted = String::from_utf8(output.stdout).unwrap();
    assert_eq!(painted, rendered(&dir, &["-p", "tty", "-O", "tty.colors=none", "-o", "-"]));
}

/// The glyphs an image drawn in the terminal is made of.
const GLYPHS: [char; 4] = ['▀', '▄', '█', ' '];

/// The document of the image tests, which turns the graphics feature on: an image shown as
/// a block, the content of a figure, written `\includegraphics[OPTIONS]{PATH}`.
fn figure(path: &str, options: &str) -> String {
    format!(
        "---\nflm:\n  features:\n    graphics: {{global: true}}\n---\nBefore.\n\n\\begin{{figure}}\n\
         \\includegraphics[{options}]{{{path}}}\n\\caption{{A disk}}\n\\end{{figure}}\n\nAfter.\n"
    )
}

/// A fresh folder of the test's own holding the document `document` and two pictures of 48
/// by 32 pixels at 96 dots per inch: `figures/disk.png`, a yellow disk on dark blue, and
/// `figures/pale.png`, a plain pale yellow (250, 250, 150).
fn image_folder(name: &str, document: &str) -> PathBuf {
    let dir = folder(name, document);
    std::fs::create_dir_all(dir.join("figures")).unwrap();
    for picture in ["disk.png", "pale.png"] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/figures").join(picture);
        std::fs::copy(source, dir.join("figures").join(picture)).unwrap();
    }
    dir
}

/// The rows of the drawing that `shown` prints under the line `label`: after it and the
/// empty lines that follow it, the lines up to the next empty line (a row of spaces is not
/// empty).
fn drawing<'a>(shown: &'a str, label: &str) -> Vec<&'a str> {
    let mut lines = shown.lines().skip_while(|line| line.trim() != label);
    assert!(lines.next().is_some(), "the label {label:?} prints: {shown}");
    lines.skip_while(|line| line.is_empty()).take_while(|line| !line.is_empty()).collect()
}

/// The standard output of a render that succeeded with warnings alone reported: the
/// graphics feature's, for a picture shown at another size than its own.
fn rendered_resized(dir: &Path, args: &[&str]) -> String {
    let output = render(dir, &[args, &["--json-messages"]].concat());
    let (reported, summary) = diagnostics(&output);
    assert!(output.status.success() && summary["errors"] == 0, "{args:?}: {output:?}");
    assert!(
        !reported.is_empty() && reported.iter().all(|(severity, id)| severity == "warning" && id == "flm.graphics.resized"),
        "{reported:?}"
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Checks that `rows` is a grid of `columns` by `lines` glyphs, each one of the four, one
/// at least not a space.
fn assert_grid(rows: &[&str], columns: usize, lines: usize) {
    assert_eq!(rows.len(), lines, "{rows:?}");
    for row in rows {
        assert_eq!(row.chars().count(), columns, "{rows:?}");
        assert!(row.chars().all(|c| GLYPHS.contains(&c)), "{rows:?}");
    }
    assert!(rows.iter().any(|row| row.contains(['▀', '▄', '█'])), "{rows:?}");
}

/// Whether `shown` holds a half-block glyph.
fn draws(shown: &str) -> bool {
    shown.contains(['▀', '▄', '█'])
}

/// The rule `tty.graphics.image` lowers the image node that the graphics feature's rule
/// staged in the round before: the image shown as a block prints its label line, then a
/// grid of half-block glyphs whose size is the picture's at 96 dots per inch, 36 by 24
/// points, six columns of 6 points by two lines of 12 points.
#[test]
fn an_image_shown_as_a_block_prints_its_label_and_a_grid_of_half_blocks() {
    let dir = image_folder("image-block", &figure("figures/disk.png", ""));
    let painted = rendered(&dir, &["-p", "tty", "-O", "tty.colors=none", "-o", "-"]);
    let shown = shown(&painted);
    assert_grid(&drawing(&shown, "[image: figures/disk.png]"), 6, 2);
    assert!(shown.contains("Before.") && shown.contains("A disk") && shown.contains("After."), "{shown}");
    assert_every_line_closes_what_it_opens(&painted);
}

/// The grid follows the width and the height the author gives: `width=1in` gives 12
/// columns, and the height follows the picture's ratio, two thirds of an inch, 4 lines;
/// `width=1in,height=0.5in` gives 12 columns by 3 lines. The alternative text is the label.
/// The graphics feature warns that the picture is shown at another size than its own.
#[test]
fn the_grid_follows_the_width_and_the_height_given() {
    let dir = image_folder("image-width", &figure("figures/disk.png", "width=1in"));
    let wide = shown(&rendered_resized(&dir, &["-p", "tty", "-o", "-"]));
    assert_grid(&drawing(&wide, "[image: figures/disk.png]"), 12, 4);

    let dir = image_folder("image-width-height", &figure("figures/disk.png", "alt=A disk, width=1in, height=0.5in"));
    let flat = shown(&rendered_resized(&dir, &["-p", "tty", "-o", "-"]));
    assert_grid(&drawing(&flat, "[image: A disk]"), 12, 3);
}

/// An image shown inline prints its label alone, in its line, as under `-p text`.
#[test]
fn an_inline_image_prints_its_label_alone() {
    let document = "---\nflm:\n  features:\n    graphics: {global: true}\n---\nSee \\includegraphics{figures/disk.png} here.\n";
    let dir = image_folder("image-inline", document);
    let shown = shown(&rendered(&dir, &["-p", "tty", "-o", "-"]));
    assert!(shown.contains("See [image: figures/disk.png] here.") && !draws(&shown), "{shown}");
    assert_eq!(shown, rendered(&dir, &[&["-p", "text"], TTY_SETTINGS, &["-o", "-"]].concat()));
}

/// The output `text` prints the label alone for an image shown as a block, and so does the
/// output `tty` with `-O graphics.strategy.text=reference`, which keeps no content to draw,
/// with nothing reported. The figure, of a type that accepts any content, holds two images,
/// so that the labels' layout, each on lines of its own, is the text output's too.
#[test]
fn without_the_strategy_content_a_block_image_prints_its_label_alone() {
    let document = "---\nflm:\n  features:\n    graphics: {global: true}\n    floats:\n      types:\n        \
                    figure: {handlers: [any]}\n---\nBefore.\n\n\\begin{figure}\n\\includegraphics{figures/disk.png}\n\
                    \\includegraphics[alt=A second disk]{figures/disk.png}\n\\caption{A disk}\n\\end{figure}\n\nAfter.\n";
    let dir = image_folder("image-reference", document);
    let text = rendered(&dir, &[&["-p", "text"], TTY_SETTINGS, &["-o", "-"]].concat());
    assert!(text.contains("[image: figures/disk.png]") && text.contains("[image: A second disk]"), "{text}");
    assert!(!draws(&text), "{text}");
    let shown = shown(&rendered(&dir, &["-p", "tty", "-O", "graphics.strategy.text=reference", "-o", "-"]));
    assert_eq!(shown, text);
}

/// A cell size that is not a positive number is an error of the stage, and the default size
/// applies.
#[test]
fn an_invalid_cell_size_is_an_error_and_the_default_applies() {
    let dir = image_folder("image-cell-invalid", &figure("figures/disk.png", ""));
    let args = ["-p", "tty", "-O", "tty.graphics_cell_width_pt=0", "--continue-with-errors", "--json-messages", "-o", "-"];
    let output = render(&dir, &args);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let (reported, summary) = diagnostics(&output);
    assert_eq!(reported, [common::reported("error", "tty.invalid-cell-size")], "{output:?}");
    assert_eq!(summary["errors"], 1, "{summary}");
    let painted = String::from_utf8(output.stdout).unwrap();
    assert_eq!(painted, rendered(&dir, &["-p", "tty", "-o", "-"]));
    assert_grid(&drawing(&shown(&painted), "[image: figures/disk.png]"), 6, 2);

    // A valid size changes the grid: cells of 3 by 6 points give 12 columns by 4 lines.
    let args = ["-p", "tty", "-O", "tty.graphics_cell_width_pt=3", "-O", "tty.graphics_cell_height_pt=6", "-o", "-"];
    assert_grid(&drawing(&shown(&rendered(&dir, &args)), "[image: figures/disk.png]"), 12, 4);
}

/// An SVG image, with no `magick` program to convert it to PNG: the publish stage reports
/// the failed conversion, the rule reports that the terminal output shows images of four
/// formats, and the label prints alone.
#[test]
fn an_svg_image_without_a_converter_is_reported_and_prints_its_label() {
    let dir = image_folder("image-svg", &figure("figures/drawing.svg", ""));
    std::fs::write(
        dir.join("figures/drawing.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"72pt\" height=\"48pt\"><rect width=\"72\" height=\"24\"/></svg>\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(["render", "paper.flm", "-p", "tty", "--continue-with-errors", "--json-messages", "-o", "-"])
        .current_dir(&dir)
        .env("HOME", &dir)
        .env("PATH", dir.join("bin"))
        .env_remove("MAGICK")
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let (reported, summary) = diagnostics(&output);
    let error = |id: &str| ("error".to_owned(), id.to_owned());
    assert_eq!(reported, [error("flm.graphics.conversion-failed"), error("tty.graphics-not-shown")]);
    assert_eq!((&summary["errors"], &summary["warnings"]), (&Value::from(2), &Value::from(0)), "{summary}");
    let shown = shown(&String::from_utf8(output.stdout).unwrap());
    assert!(shown.contains("[image: figures/drawing.svg]") && !draws(&shown), "{shown}");
}

/// With ImageMagick installed (gated: `FLM_TESTS_MAGICK=1`), an SVG image is converted to
/// PNG and drawn at its stated size, 72 by 48 points: 12 columns by 4 lines. Otherwise the
/// test writes one line beginning `skipped:`.
#[test]
fn with_imagemagick_an_svg_image_is_drawn_at_its_stated_size() {
    if std::env::var_os("FLM_TESTS_MAGICK").is_none_or(|value| value != "1") {
        println!("skipped: set FLM_TESTS_MAGICK=1 to run the test with ImageMagick's `magick` on PATH");
        return;
    }
    let dir = image_folder("image-svg-magick", &figure("figures/drawing.svg", ""));
    std::fs::write(
        dir.join("figures/drawing.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"72pt\" height=\"48pt\" viewBox=\"0 0 72 48\">\
         <rect width=\"72\" height=\"48\" fill=\"white\"/><rect width=\"36\" height=\"48\" fill=\"black\"/></svg>\n",
    )
    .unwrap();
    let shown = shown(&rendered(&dir, &["-p", "tty", "-o", "-"]));
    assert_grid(&drawing(&shown, "[image: figures/drawing.svg]"), 12, 4);
}

/// The painted lines of the drawing printed under the line `label`, as [`drawing`] finds
/// them in the text shown.
fn painted_drawing<'a>(painted: &'a str, label: &str) -> Vec<&'a str> {
    let mut lines = painted.lines().skip_while(|line| shown(line).trim() != label);
    assert!(lines.next().is_some(), "the label {label:?} prints: {painted}");
    lines.skip_while(|line| line.is_empty()).take_while(|line| !line.is_empty()).collect()
}

/// Whether one of the SGR parameters `parameters` sets a standard color of the text or of
/// the background.
fn sets_a_standard_color(parameters: &str) -> bool {
    parameters.split(';').any(|p| p.parse::<u16>().is_ok_and(|p| matches!(p, 30..=37 | 40..=47 | 90..=97 | 100..=107)))
}

/// The drawing is colored by default, with standard colors; with colors given by their
/// values under `rich`; and not at all under `none`. The characters are the same in the
/// three, the label line carries no sequence, and every line closes what it opens.
#[test]
fn the_drawing_is_colored_by_default_with_values_under_rich_and_not_under_none() {
    let dir = image_folder("image-colors", &figure("figures/disk.png", ""));
    let label = "[image: figures/disk.png]";
    let standard = rendered(&dir, &["-p", "tty", "-o", "-"]);
    let rich = rendered(&dir, &["-p", "tty", "-O", "tty.colors=rich", "-o", "-"]);
    let none = rendered(&dir, &["-p", "tty", "-O", "tty.colors=none", "-o", "-"]);
    assert_eq!(shown(&standard), shown(&none));
    assert_eq!(shown(&rich), shown(&none));
    for line in painted_drawing(&standard, label) {
        let parameters = sgr_parameters(line);
        assert!(parameters.iter().any(|p| sets_a_standard_color(p)), "{line:?}");
        assert!(parameters.iter().all(|p| !p.contains(";2;")), "{line:?}");
    }
    for line in painted_drawing(&rich, label) {
        assert!(sgr_parameters(line).iter().any(|p| p.contains(";38;2;") || p.contains(";48;2;")), "{line:?}");
    }
    let rows = painted_drawing(&none, label);
    assert_grid(&rows, 6, 2);
    assert!(rows.iter().all(|line| scan(line).1.is_empty()), "{rows:?}");
    for painted in [&standard, &rich] {
        let label_line = painted.lines().find(|line| shown(line).trim() == label).unwrap();
        assert!(scan(label_line).1.is_empty(), "{label_line:?}");
        assert_every_line_closes_what_it_opens(painted);
    }
}

/// Two pictures of different content on one page are each colored from their own content:
/// the plain pale yellow is drawn in full blocks of its color, and the disk shows none of
/// it.
#[test]
fn two_images_are_each_colored_from_their_own_content() {
    let document = "---\nflm:\n  features:\n    graphics: {global: true}\n---\n\\begin{figure}\n\
                    \\includegraphics{figures/disk.png}\n\\caption{A disk}\n\\end{figure}\n\n\\begin{figure}\n\
                    \\includegraphics[alt=Pale]{figures/pale.png}\n\\caption{A pale square}\n\\end{figure}\n";
    let dir = image_folder("image-two", document);
    let painted = rendered(&dir, &["-p", "tty", "-O", "tty.colors=rich", "-o", "-"]);
    let pale = painted_drawing(&painted, "[image: Pale]");
    assert_eq!(pale.len(), 2, "{pale:?}");
    for line in &pale {
        assert_eq!(shown(line).trim_start(), "██████");
        assert_eq!(sgr_parameters(line), ["0;38;2;250;250;150", "0"]);
    }
    let disk = painted_drawing(&painted, "[image: figures/disk.png]");
    let disk_shown: Vec<String> = disk.iter().map(|line| shown(line)).collect();
    assert_grid(&disk_shown.iter().map(String::as_str).collect::<Vec<_>>(), 6, 2);
    for line in &disk {
        let parameters = sgr_parameters(line);
        assert!(parameters.iter().any(|p| p.contains(";2;")), "{line:?}");
        assert!(parameters.iter().all(|p| !p.contains("250;250;150")), "{line:?}");
    }
}

/// Painting changes no character: the output `tty` with its escape sequences removed is the
/// same under each value of `tty.colors`, and is the output `text` under the settings of
/// the workflow `tty` and the strategy `content` with the drawing's rows added under the
/// label, which the output `text` prints alone.
#[test]
fn the_painted_text_is_the_text_output_with_the_drawing_added() {
    let dir = image_folder("image-text", &figure("figures/disk.png", ""));
    let label = "[image: figures/disk.png]";
    let none = shown(&rendered(&dir, &["-p", "tty", "-O", "tty.colors=none", "-o", "-"]));
    for colors in ["standard", "rich"] {
        assert_eq!(shown(&rendered(&dir, &["-p", "tty", "-O", &format!("tty.colors={colors}"), "-o", "-"])), none);
    }
    let text =
        rendered(&dir, &[&["-p", "text"], TTY_SETTINGS, &["-O", "graphics.strategy.text=content", "-o", "-"]].concat());
    assert!(!draws(&text), "{text}");
    let rows = drawing(&none, label);
    assert_grid(&rows, 6, 2);
    // The text with the drawing's rows, and the blank line before them, removed.
    let mut lines: Vec<&str> = none.lines().collect();
    let first = lines.iter().position(|line| *line == rows[0]).unwrap();
    let blank = first - 1;
    assert!(lines[blank].is_empty() && lines[blank - 1].trim() == label, "{none}");
    lines.drain(blank..first + rows.len());
    assert_eq!(lines.join("\n") + "\n", text);
}

/// A verbatim block an author wrote is never colored, even when it holds the very glyphs of
/// a drawing on the same page: only the drawing, the verbatim block of the image's own
/// block, is.
#[test]
fn a_verbatim_block_an_author_wrote_is_never_colored() {
    let document = "---\nflm:\n  features:\n    graphics: {global: true}\n---\n\\begin{figure}\n\
                    \\includegraphics[alt=Pale]{figures/pale.png}\n\\caption{A pale square}\n\\end{figure}\n\n\
                    \\begin{verbatimtext}\n██████\n██████\n\\end{verbatimtext}\n";
    let dir = image_folder("image-verbatim", document);
    let painted = rendered(&dir, &["-p", "tty", "-O", "tty.colors=rich", "-o", "-"]);
    let blocks: Vec<&str> = painted.lines().filter(|line| shown(line).trim() == "██████").collect();
    assert_eq!(blocks.len(), 4, "{painted}");
    assert_eq!(painted_drawing(&painted, "[image: Pale]"), blocks[..2]);
    for line in &blocks[..2] {
        assert_eq!(sgr_parameters(line), ["0;38;2;250;250;150", "0"]);
    }
    for line in &blocks[2..] {
        assert!(scan(line).1.is_empty(), "{line:?}");
    }
}

#[cfg(feature = "wasm")]
#[path = "../../flm-ext/tests/common/guest.rs"]
#[allow(dead_code)]
mod guest;

/// The extension's WebAssembly form, built by `extensions/rust/build-wasm.sh` into a folder
/// that `--extensions-dir` points at, is found before the extension compiled in and gives
/// the same bytes for the document.
#[cfg(feature = "wasm")]
#[test]
fn the_webassembly_form_built_by_the_script_gives_the_same_bytes() {
    if let Some(line) = guest::skipped() {
        println!("{line}");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let modules = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-tty-wasm");
    let _ = std::fs::remove_dir_all(&modules);
    // The script's cargo run builds in a target folder of its own, apart from the folder
    // of the cargo run that runs this test.
    let script = Command::new(root.join("extensions/rust/build-wasm.sh"))
        .arg("--out")
        .arg(&modules)
        .arg("profile-tty")
        .env("CARGO_TARGET_DIR", root.join("target/wasm-guest"))
        .output()
        .unwrap();
    assert!(script.status.success(), "the script must build the module:\n{}", String::from_utf8_lossy(&script.stderr));
    let mut shipped: Vec<String> =
        std::fs::read_dir(modules.join("profile-tty")).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    shipped.sort();
    assert_eq!(shipped, ["README.md", "flmextension.yaml", "profile_tty.wasm"]);

    let dir = folder("wasm", PAPER);
    let native = rendered(&dir, &["-p", "tty", "-o", "-"]);
    let output = render(&dir, &["--extensions-dir", modules.to_str().unwrap(), "-v", "-p", "tty", "-o", "-"]);
    let said = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{said}");
    // The module was loaded, not the extension compiled in (`-v` lists the packages in use).
    assert!(said.contains(modules.join("profile-tty").to_str().unwrap()) && !said.contains("bundled:rust/profile-tty"), "{said}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), native);

    // The drawings and their colors, given by their values.
    let dir = image_folder("wasm-images", &figure("figures/disk.png", ""));
    let args = ["-p", "tty", "-O", "tty.colors=rich", "-o", "-"];
    let native = rendered(&dir, &args);
    assert!(native.contains(";38;2;"), "{native}");
    let output = render(&dir, &[&["--extensions-dir", modules.to_str().unwrap()][..], &args].concat());
    let said = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{said}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), native);
}
