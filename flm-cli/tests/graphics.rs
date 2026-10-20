//! Images through the program: a document showing a PNG image, rendered to the standard
//! output, and to a file in another folder than the document's under the strategies `copy`
//! and `reference`; the images read and looked for, listed by `-v`; a PDF figure made into a
//! PNG image by the converter `magick`, whose program is a stand-in of the test's own, named
//! by the environment variable `MAGICK`. Every file is written under the build folder, and
//! no test starts ImageMagick's real program.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh folder `name` under the build folder, holding `doc/paper.flm`, which shows
/// `figures/plot.png`, and that image.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-graphics-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("doc/figures")).unwrap();
    std::fs::write(
        path.join("doc/paper.flm"),
        "---\nflm:\n  features:\n    graphics: {global: true}\n---\nA plot: \\includegraphics{figures/plot.png}.\n",
    )
    .unwrap();
    let image = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/figures/plot.png");
    std::fs::copy(image, path.join("doc/figures/plot.png")).unwrap();
    path
}

/// Runs `flm` with `args` in `cwd`, and asserts that it succeeded and reported nothing.
fn flm(cwd: &Path, args: &[&str]) -> Output {
    flm_run(Command::new(env!("CARGO_BIN_EXE_flm")).args(args).current_dir(cwd), args)
}

/// Runs `command`, the program `flm` with `args`, and asserts that it succeeded and
/// reported nothing.
fn flm_run(command: &mut Command, args: &[&str]) -> Output {
    let output = command.output().unwrap();
    let reported = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{args:?}: {reported}");
    assert!(reported.is_empty(), "{args:?}: {reported}");
    output
}

/// The address the page `page` writes for its one image.
fn source(page: &str) -> String {
    let image = &page[page.find("<img").expect("an image")..];
    let address = &image[image.find("src=\"").expect("an address") + 5..];
    address[..address.find('"').unwrap()].to_string()
}

#[test]
fn a_page_on_the_standard_output_holds_its_image() {
    let dir = folder("stdout");
    let output = flm(&dir, &["render", "doc/paper.flm"]);
    let page = String::from_utf8(output.stdout).unwrap();
    assert!(source(&page).starts_with("data:image/png;base64,iVBORw0KGgo"), "{page}");
}

#[test]
fn under_copy_the_page_in_another_folder_leads_to_the_copy_written() {
    let dir = folder("copy");
    flm(&dir, &["render", "doc/paper.flm", "-O", "graphics.strategy.html=copy", "-o", "site/page.html"]);
    let page = std::fs::read_to_string(dir.join("site/page.html")).unwrap();
    let address = source(&page);
    assert!(address.starts_with("page-plot-") && address.ends_with(".png"), "{address}");
    let original = std::fs::read(dir.join("doc/figures/plot.png")).unwrap();
    assert_eq!(std::fs::read(dir.join("site").join(&address)).unwrap(), original);
}

#[test]
fn under_reference_the_page_in_another_folder_leads_to_the_image_where_it_stands() {
    let dir = folder("reference");
    flm(&dir, &["render", "doc/paper.flm", "-O", "graphics.strategy.html=reference", "-o", "site/page.html"]);
    let page = std::fs::read_to_string(dir.join("site/page.html")).unwrap();
    assert_eq!(source(&page), "../doc/figures/plot.png");
    // From a folder above the current one, the address climbs back through its name.
    let doc = dir.join("doc");
    flm(&doc, &["render", "paper.flm", "-O", "graphics.strategy.html=reference", "-o", "../out/page.html"]);
    let page = std::fs::read_to_string(dir.join("out/page.html")).unwrap();
    assert_eq!(source(&page), "../doc/figures/plot.png");
    assert!(dir.join("out").join(source(&page)).is_file());
}

#[test]
fn verbose_lists_the_images_read_and_looked_for() {
    let dir = folder("verbose");
    std::fs::write(
        dir.join("doc/paper.flm"),
        "---\nflm:\n  features:\n    graphics: {global: true}\n---\n\\includegraphics{figures/plot.png} \\includegraphics{figures/absent.png}\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flm")).args(["render", "doc/paper.flm", "-v"]).current_dir(&dir).output().unwrap();
    let reported = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = reported.lines().collect();
    let found = Path::new("doc").join("figures").join("plot.png");
    let absent = Path::new("doc").join("figures").join("absent.png");
    assert!(lines.contains(&format!("flm: read {}", found.display()).as_str()), "{reported}");
    assert!(lines.contains(&format!("flm: read {} (not found)", absent.display()).as_str()), "{reported}");
}

/// A PDF file of one page of 200 × 100 points.
fn pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
    ];
    let mut text = String::from("%PDF-1.6\n");
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(text.len());
        text.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = text.len();
    text.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for offset in offsets {
        text.push_str(&format!("{offset:010} 00000 n \n"));
    }
    text.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1));
    text.into_bytes()
}

#[cfg(unix)]
#[test]
fn a_page_holds_a_pdf_figure_as_the_png_image_the_converter_program_wrote() {
    use std::os::unix::fs::PermissionsExt;

    let dir = folder("magick");
    std::fs::write(
        dir.join("doc/paper.flm"),
        "---\nflm:\n  features:\n    graphics: {global: true}\n---\nA figure: \\includegraphics{figures/figure.pdf}.\n",
    )
    .unwrap();
    std::fs::write(dir.join("doc/figures/figure.pdf"), pdf()).unwrap();
    // The stand-in for `magick` writes the PNG image kept with the tests, whatever it is given.
    let png = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/figures/plot.png");
    let program = dir.join("magick");
    std::fs::write(&program, format!("#!/bin/sh\ncat > /dev/null\ncat '{}'\n", png.display())).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let args = ["render", "doc/paper.flm"];
    let output = flm_run(
        Command::new(env!("CARGO_BIN_EXE_flm")).args(args).current_dir(&dir).env("MAGICK", &program),
        &args,
    );
    let page = String::from_utf8(output.stdout).unwrap();
    let expected = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, std::fs::read(png).unwrap());
    assert_eq!(source(&page), format!("data:image/png;base64,{expected}"));
}

