//! The `math-mathjax` extension of the workspace's folder `extensions/js/`, as a user installs
//! and uses it: a document on disk whose front matter uses `math-mathjax`, prepared with
//! that folder on the search path (what the command line's `--extensions-dir` gives), read
//! through the host's filesystem source, and rendered as an HTML page in the extension's
//! `mathml` mode.

#![cfg(feature = "js")]

use std::path::PathBuf;
use std::sync::Arc;

use flm_main::files::DiskFiles;
use flm_main::output::{MemorySink, Sink};
use flm_main::packages::ExtensionFolder;
use flm_main::session::{Input, Request, build, prepare};

/// The workspace's folder of extensions.
const EXTENSIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../extensions/js");

/// The HTML page of the document `text`, written to the file `name` in a folder of the
/// tests' own, prepared with [`EXTENSIONS`] on the search path; and the build's
/// diagnostics, as `identifier: message`.
fn page(name: &str, text: &str) -> (String, Vec<String>) {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("math-mathjax");
    std::fs::create_dir_all(&folder).expect("the folder is made");
    let path = folder.join(name);
    std::fs::write(&path, text).expect("the document is written");
    let mut request = Request::new(Input::File(path));
    request.format = Some(String::from("html"));
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from(EXTENSIONS), required: true }];
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = prepare(request, Arc::new(DiskFiles), &[]).expect("prepared");
    let built = build(&prepared).expect("built");
    let diagnostics =
        built.diagnostics.iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    let entry = sink.files().into_values().find(|file| file.entry).expect("an output");
    (String::from_utf8(entry.bytes).expect("the page is text"), diagnostics)
}

/// A document using `math-mathjax` in `mathml` mode, with a user definition used inside a formula, an
/// `align` whose first line has a label and whose second line holds a reference to it
/// inside `\text{…}`, and the same reference inside an inline formula.
const DOCUMENT: &str = "---\nflm:\n  use: [math-mathjax]\n  features:\n    math-mathjax:\n      mode: mathml\n    \
                        userdefs:\n      macros:\n        sq: {content: 'x^2'}\n---\n\
                        The square \\(\\sq\\).\n\n\
                        \\begin{align}\n  a &= b \\label{eq:a} \\\\\n  c &= d \\text{ by \\ref{eq:a}} \\label{eq:c}\n\\end{align}\n\n\
                        See \\(\\text{see \\ref{eq:a}}\\).\n";

#[test]
fn a_document_using_the_extension_from_the_folder_renders_its_formulas_as_mathml() {
    let (page, diagnostics) = page("paper.flm", DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The user definition expanded before MathJax read the formula.
    assert!(
        page.contains(r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#),
        "{page}"
    );
    // The labeled line's row, its number in the last cell, and the reference's link
    // inside an `mtext`.
    assert!(page.contains(r#"<mtr id="eq-a">"#), "{page}");
    assert!(
        page.contains(concat!(
            r#"<mtr id="eq-c"><mtd class="math-mathjax-tag-space"></mtd>"#,
            r#"<mtd style="text-align: right; padding-left: 0; padding-right: 0em; padding-top: 1.5pt; padding-bottom: 0"><mi>c</mi></mtd>"#,
            r#"<mtd style="text-align: left; padding-left: 0em; padding-right: 0; padding-top: 1.5pt; padding-bottom: 0">"#,
            r#"<mstyle indentshift="2em"><mi></mi><mspace width="0.278em"></mspace><mo lspace="0" rspace="0">=</mo>"#,
            r#"<mspace width="0.278em"></mspace><mi>d</mi><mtext><span class="flm-pres-math--text" data-flm-command="text"> by "#,
            r##"<a class="flm-pres-link" href="#eq-a" data-flm-kind="ref">Eq.&#xA0;(1)</a></span></mtext></mstyle></mtd>"##,
            r#"<mtd class="math-mathjax-tag" style="padding-top: 1.5pt; padding-bottom: 0"><mtext>(2)</mtext></mtd></mtr>"#,
        )),
        "{page}"
    );
    assert!(page.contains(r#"<mtd class="math-mathjax-tag" style="padding-top: 0; padding-bottom: 1.5pt"><mtext>(1)</mtext></mtd></mtr>"#), "{page}");
    assert!(
        page.contains(r##"<mtext><span class="flm-pres-math--text" data-flm-command="text">see <a class="flm-pres-link" href="#eq-a""##),
        "{page}"
    );
    // The extension's stylesheet; no script, no passthrough formula (the list `[std]` loads
    // no `math-passthrough`).
    assert!(page.contains(r#"data-flm-def="math-mathjax.style""#), "{page}");
    assert!(!page.contains("<script"), "{page}");
    assert!(!page.contains(r#"class="flm-pres-math--passthrough""#), "{page}");
}

#[test]
fn a_page_without_a_formula_carries_no_stylesheet_of_the_extension() {
    let (page, diagnostics) = page("plain.flm", "---\nflm:\n  use: [math-mathjax]\n---\nNo formula here.\n");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains("No formula here."), "{page}");
    // The stylesheet comes with the formulas that use it.
    assert!(!page.contains("math-mathjax.style"), "{page}");
}
