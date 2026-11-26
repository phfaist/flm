//! The public letter extension: optional structured fields, output-specific logos,
//! layout options, and customization files resolved beside the document.
#![cfg(all(feature = "js", feature = "bundled-extensions"))]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn folder(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-letter-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("source")).unwrap();
    dir
}

fn document(profile: &str, config: Value, content: &str) -> String {
    let mut front = json!({"title": "A letter", "flm": {"use": ["letter"], "features": {"compose": {"template_config": {}}}}});
    front["flm"]["features"]["compose"]["template_config"][profile] = json!({"letter": config});
    format!("---\n{front}\n---\n{content}\n")
}

fn render_command(dir: &Path, profile: &str) -> Command {
    let extension = match profile { "html" => "html", "latex-rendered" => "tex", "text" => "txt", _ => unreachable!() };
    let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
    command
        .args(["render", "--json-messages", "-p", profile, "source/letter.flm", "-o", &format!("out/letter.{extension}")])
        .current_dir(dir)
        .env_remove("FLMPATH");
    command
}

fn render(dir: &Path, profile: &str) -> Output {
    render_command(dir, profile).output().unwrap()
}

fn write_and_render(dir: &Path, profile: &str, config: Value, content: &str) -> Output {
    std::fs::write(dir.join("source/letter.flm"), document(profile, config, content)).unwrap();
    render(dir, profile)
}

fn clean(output: &Output) {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(common::diagnostics(output).0.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
}

fn entry(dir: &Path, profile: &str) -> String {
    let extension = match profile { "html" => "html", "latex-rendered" => "tex", "text" => "txt", _ => unreachable!() };
    std::fs::read_to_string(dir.join(format!("out/letter.{extension}"))).unwrap()
}

const BARE: &str = "\\begin{letter}\nBody marker.\n\\end{letter}";
const FIELDS: [(&str, &str); 8] = [
    ("fromaddress", "Sender marker\\\\Street 14"),
    ("toaddress", "Recipient marker\\\\Somewhere"),
    ("letterdate", "Date marker"),
    ("lettersubject", "Subject marker"),
    ("salutation", "Salutation marker"),
    ("closing", "Closing marker"),
    ("signature", "Signature marker\\\\Title marker"),
    ("enclosed", "Enclosure marker"),
];

fn letter(fields: &[(&str, &str)]) -> String {
    let mut content = String::from("\\begin{letter}\n");
    for &(name, value) in fields {
        if matches!(name, "fromaddress" | "toaddress" | "signature") {
            content.push_str(&format!("\\begin{{{name}}}{value}\\end{{{name}}}\n"));
        } else {
            content.push_str(&format!("\\{name}{{{value}}}\n"));
        }
    }
    content.push_str("\nBody marker with \\emph{emphasis}.\n\nSecond paragraph.\n\\end{letter}");
    content
}

#[test]
fn bundled_letters_render_both_layouts_and_plain_text() {
    for style in ["europe", "us"] {
        for profile in ["html", "latex-rendered", "text"] {
            let dir = folder(&format!("bundled-{style}-{profile}"));
            clean(&write_and_render(&dir, profile, json!({"style": style}), &letter(&FIELDS)));
            let result = entry(&dir, profile);
            for marker in ["Sender marker", "Recipient marker", "Date marker", "Subject marker", "Body marker", "Signature marker", "Title marker", "Enclosure marker"] {
                assert!(result.contains(marker), "{style}/{profile}: missing {marker}");
            }
            assert!(result.find("Recipient marker") < result.find("Subject marker"));
            assert!(result.find("Subject marker") < result.find("Body marker"));
            assert!(result.find("Body marker") < result.find("Signature marker"));
            if profile == "html" {
                assert!(result.contains("class=\"flm-letter\""));
                assert!(!result.contains("<img"), "the default has no logo");
            }
        }
    }
}

#[test]
fn optional_fields_are_reordered_and_empty_fields_disappear() {
    let mut reversed = FIELDS;
    reversed.reverse();
    for profile in ["html", "latex-rendered"] {
        let dir = folder(&format!("reordered-{profile}"));
        clean(&write_and_render(&dir, profile, json!({}), &letter(&reversed)));
        let result = entry(&dir, profile);
        assert!(result.find("Sender marker") < result.find("Recipient marker"));
        assert!(result.find("Recipient marker") < result.find("Body marker"));
        assert!(result.find("Signature marker") < result.find("Enclosure marker"));

        for (index, fields) in [
            vec![],
            vec![("closing", "Closing marker")],
            vec![("signature", "Signature marker")],
            vec![("closing", "Closing marker"), ("signature", "Signature marker")],
            vec![("toaddress", "  %% comment only\n"), ("closing", "  "), ("signature", "  %% comment only\n")],
        ].into_iter().enumerate() {
            let dir = folder(&format!("optional-{profile}-{index}"));
            clean(&write_and_render(&dir, profile, json!({}), &letter(&fields)));
            let result = entry(&dir, profile);
            for marker in ["Closing marker", "Signature marker"] {
                assert_eq!(result.contains(marker), fields.iter().any(|(_, value)| value.contains(marker)));
            }
            assert!(!result.contains("Recipient marker"));
            if profile == "html" {
                assert!(!result.contains("class=\"flm-letter-toaddress\""));
            }
        }
    }
}

#[test]
fn fields_preserve_rich_content_and_multiline_enclosures() {
    let content = r"\begin{letter}
\lettersubject{A \emph{formatted} subject with \(x^2\)}
\begin{fromaddress}
Sender marker\\
\email{sender@example.org}
\end{fromaddress}
\salutation{Dear \emph{reader},}

Body marker with \href{https://example.org/letter}{a link} and a note.\footnote{NoteMarker.}

\begin{enclosed}
First attachment\\
Second attachment
\end{enclosed}
\end{letter}";
    for profile in ["html", "latex-rendered", "text"] {
        let dir = folder(&format!("rich-{profile}"));
        clean(&write_and_render(&dir, profile, json!({}), content));
        let result = entry(&dir, profile);
        for marker in ["formatted", "sender@example.org", "Body marker", "First attachment", "Second attachment"] {
            assert!(result.contains(marker), "{profile}: {marker}");
        }
        assert_eq!(result.matches("NoteMarker").count(), 1, "{profile}: a note is rendered once");
        if profile == "html" {
            assert!(result.contains("href=\"https://example.org/letter\""));
            assert!(result.contains("href=\"mailto:sender@example.org\""));
        }
    }
}

#[test]
fn letter_structure_reports_duplicate_misplaced_and_nested_fields() {
    let duplicate_enclosed = "\\begin{letter}\\enclosed{One}\\begin{enclosed}Two\\end{enclosed}\\end{letter}";
    let cases = [
        ("duplicate", "\\begin{letter}\\closing{One}\\closing{Two}\\end{letter}", "letter.duplicate-field"),
        ("duplicate-forms", duplicate_enclosed, "letter.duplicate-field"),
        ("misplaced", "\\closing{Outside}\n\\begin{letter}Body.\\end{letter}", "letter.misplaced-field"),
        ("nested-field", "\\begin{letter}\\begin{fromaddress}\\begin{toaddress}Nested\\end{toaddress}\\end{fromaddress}\\end{letter}", "letter.misplaced-field"),
        ("nested-letter", "\\begin{letter}\\begin{letter}Nested\\end{letter}\\end{letter}", "letter.letter-count"),
        ("multiple", "\\begin{letter}One\\end{letter}\\begin{letter}Two\\end{letter}", "letter.letter-count"),
        ("missing", "No letter here.", "letter.letter-count"),
        ("wrapped", "\\begin{quote}\\begin{letter}Body.\\end{letter}\\end{quote}", "letter.misplaced-letter"),
    ];
    for (name, content, diagnostic) in cases {
        let dir = folder(&format!("structure-{name}"));
        let output = write_and_render(&dir, "html", json!({}), content);
        assert!(!output.status.success(), "{name}");
        assert!(common::diagnostics(&output).0.contains(&common::reported("error", diagnostic)), "{name}: {}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn template_options_reject_invalid_types_lengths_and_geometry() {
    let invalid = [
        json!({"style": "other"}), json!({"paper": "legal"}), json!({"letterhead": "other"}),
        json!({"logo": false}), json!({"logo": ""}),
        json!({"font_size": "0pt"}), json!({"font_size": "1em"}), json!({"font_size": ["11pt"]}),
        json!({"font_size": {"size": "11pt"}}), json!({"line_spacing": true}),
        json!({"line_spacing": "0"}), json!({"line_spacing": "0.5"}), json!({"line_spacing": "Infinity"}), json!({"line_spacing": "6"}),
        json!({"margin_left": "-1mm"}), json!({"margin_left": "200mm"}), json!({"margin_top": "290mm"}),
        json!({"letterhead_margin_top": "-1mm"}), json!({"letterhead_margin_top": "260mm"}),
        json!({"signature_gap": "1mm; color:red"}), json!({"header_gap": null}),
        json!({"paragraph_indent": -1}), json!({"logo": "placeholder", "logo_width": "200mm"}),
        json!({"logo": "placeholder", "logo_max_height": "0mm"}),
    ];
    let html = [json!({"logo_alt": null}), json!({"show_page_breaks": "yes"}), json!({"custom_css": false}), json!({"custom_css_file": 1})];
    let latex = [json!({"latex_preamble": null}), json!({"latex_preamble_file": 1})];
    for (profile, own) in [("html", &html[..]), ("latex-rendered", &latex[..])] {
        for (index, config) in invalid.iter().chain(own).enumerate() {
            let dir = folder(&format!("invalid-{profile}-{index}"));
            let output = write_and_render(&dir, profile, config.clone(), BARE);
            assert!(!output.status.success(), "{profile}: {config}");
            assert!(common::diagnostics(&output).0.contains(&common::reported("error", "flm.compose.template-failed")), "{config}: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
}

#[test]
fn each_output_declares_only_its_own_customization_options() {
    for (profile, other) in [
        ("html", json!({"latex_preamble": "\\Marker", "latex_preamble_file": "./letter.tex"})),
        ("latex-rendered", json!({"custom_css": "Marker", "custom_css_file": "./letter.css", "logo_alt": "Marker", "show_page_breaks": true})),
    ] {
        let dir = folder(&format!("other-output-{profile}"));
        let output = write_and_render(&dir, profile, other.clone(), BARE);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let unknown = common::diagnostics_of(&output, "flm.host.unknown-setting");
        assert_eq!(unknown.len(), other.as_object().unwrap().len(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(!entry(&dir, profile).contains("Marker"), "an option of the other output is ignored");
    }
}

#[test]
fn documented_geometry_boundaries_and_inactive_logo_options_are_valid() {
    for profile in ["html", "latex-rendered"] {
        let dir = folder(&format!("geometry-boundary-{profile}"));
        let config = json!({"font_size": "6pt", "line_spacing": 1,
            "margin_left": "170mm", "margin_right": "0mm", "margin_top": "257mm", "margin_bottom": "0mm",
            "paragraph_indent": "30mm", "logo": null, "logo_width": "0mm", "logo_max_height": "0mm"});
        clean(&write_and_render(&dir, profile, config, BARE));
        assert!(entry(&dir, profile).contains("Body marker"));
    }
}

#[test]
fn wide_letterheads_and_em_lengths_work_for_both_papers() {
    for profile in ["html", "latex-rendered"] {
        for (style, paper) in [("europe", "a4"), ("us", "letter")] {
            for letterhead in ["columns", "text-width", "page-width"] {
                let dir = folder(&format!("wide-{profile}-{style}-{letterhead}"));
                let config = json!({"style": style, "paper": paper, "logo": "placeholder", "letterhead": letterhead,
                    "font_size": "12pt", "line_spacing": "1.4", "margin_left": "3em", "header_gap": "1em",
                    "paragraph_spacing": "0pt", "paragraph_indent": "0pt", "signature_gap": "0mm"});
                clean(&write_and_render(&dir, profile, config, &letter(&FIELDS)));
                let result = entry(&dir, profile);
                assert!(result.contains("Body marker"));
                let extension = if profile == "html" { "svg" } else { "pdf" };
                let assets = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
                assert!(assets.iter().any(|path| path.extension().is_some_and(|actual| actual == extension)), "{assets:?}");
            }
        }
    }
}

#[test]
fn logos_are_read_beside_the_document_and_copied_as_bytes() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="80"><path fill="#385070" d="M0 0h600v80H0z"/></svg>"##;
    let dir = folder("document-logo");
    std::fs::write(dir.join("source/my letterhead.svg"), svg).unwrap();
    clean(&write_and_render(&dir, "html", json!({"logo": "./my letterhead.svg", "logo_alt": "Company & research \"group\" </style><script>bad()</script>", "letterhead": "page-width"}), BARE));
    let page = entry(&dir, "html");
    assert!(page.contains("aria-label=\"Company &amp; research &quot;group&quot;"));
    assert!(page.contains("class=\"flm-letter-logo-description\" role=\"img\""));
    assert!(!page.contains("Company & research \"group\""), "alternative text is escaped as an HTML attribute");
    assert!(!page.contains("<script>bad()</script>"), "alternative text cannot close the stylesheet");
    let copied = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "svg")).unwrap();
    assert_eq!(std::fs::read(copied).unwrap(), svg);

    let dir = folder("document-logo-pdf");
    let pdf = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../extensions/js/letter/assets/placeholder.pdf")).unwrap();
    std::fs::write(dir.join("source/my letterhead.pdf"), &pdf).unwrap();
    clean(&write_and_render(&dir, "latex-rendered", json!({"logo": "./my letterhead.pdf"}), BARE));
    let copied = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "pdf")).unwrap();
    assert_eq!(std::fs::read(copied).unwrap(), pdf);
}

#[test]
fn combined_outputs_keep_distinct_png_logos_and_their_references() {
    let dir = folder("combined-logos");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/figures");
    let html_logo = std::fs::read(fixtures.join("plot.png")).unwrap();
    let latex_logo = std::fs::read(fixtures.join("pale.png")).unwrap();
    assert_ne!(html_logo, latex_logo);
    std::fs::write(dir.join("source/web-logo.png"), &html_logo).unwrap();
    std::fs::write(dir.join("source/print-logo.png"), &latex_logo).unwrap();
    let extension = dir.join("source/_flmextensions/dual");
    std::fs::create_dir_all(&extension).unwrap();
    std::fs::write(extension.join("flmextension.yaml"),
        "extension: {description: Two letter outputs}\nflm:\n  workflows:\n    dual: {extends: html, outputs: [html, latex-rendered]}\n").unwrap();
    let front = json!({"flm": {
        "use": ["letter", "dual"],
        "features": {"compose": {"template_config": {
            "html": {"letter": {"logo": "./web-logo.png"}},
            "latex-rendered": {"letter": {"logo": "./print-logo.png"}}
        }}}
    }});
    std::fs::write(dir.join("source/letter.flm"), format!("---\n{front}\n---\n{BARE}\n")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(["render", "--json-messages", "-p", "dual", "source/letter.flm", "-o", "out/letter.html"])
        .current_dir(&dir).env_remove("FLMPATH").output().unwrap();
    clean(&output);

    let pngs = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "png")).collect::<Vec<_>>();
    assert_eq!(pngs.len(), 2, "both logos are emitted under distinct paths");
    let html_path = pngs.iter().find(|path| std::fs::read(path).unwrap() == html_logo).unwrap();
    let latex_path = pngs.iter().find(|path| std::fs::read(path).unwrap() == latex_logo).unwrap();
    assert_ne!(html_path, latex_path);
    let html_reference = html_path.file_name().unwrap().to_str().unwrap();
    let latex_reference = latex_path.file_name().unwrap().to_str().unwrap();
    let html = entry(&dir, "html");
    let latex = entry(&dir, "latex-rendered");
    assert!(html.contains(html_reference) && !html.contains(latex_reference));
    assert!(latex.contains(latex_reference) && !latex.contains(html_reference));
}

#[test]
fn separate_svg_and_pdf_logos_are_selected_for_each_output() {
    let dir = folder("separate-logo-versions");
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="80"><path fill="#385070" d="M0 0h600v80H0z"/></svg>"##;
    let pdf = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../extensions/js/letter/assets/placeholder.pdf")).unwrap();
    std::fs::write(dir.join("source/my letterhead.svg"), svg).unwrap();
    let front = json!({"flm": {"use": ["letter"], "features": {"compose": {"template_config": {
        "html": {"letter": {"logo": "./my letterhead.svg"}},
        "latex-rendered": {"letter": {"logo": "./my letterhead.pdf"}}
    }}}}});
    std::fs::write(dir.join("source/letter.flm"), format!("---\n{front}\n---\n{BARE}\n")).unwrap();

    // A render needs only its own logo, even when another output is configured.
    clean(&render(&dir, "html"));
    let copied_svg = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "svg")).unwrap();
    assert_eq!(std::fs::read(copied_svg).unwrap(), svg);

    std::fs::remove_file(dir.join("source/my letterhead.svg")).unwrap();
    std::fs::write(dir.join("source/my letterhead.pdf"), &pdf).unwrap();
    clean(&render(&dir, "latex-rendered"));
    let copied_pdf = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "pdf")).unwrap();
    assert_eq!(std::fs::read(&copied_pdf).unwrap(), pdf);
    assert!(entry(&dir, "latex-rendered").contains(copied_pdf.file_name().unwrap().to_str().unwrap()), "the LaTeX refers to the emitted PDF");
}

#[test]
fn customization_files_precede_inline_overrides_and_follow_default_styles() {
    for profile in ["html", "latex-rendered"] {
        let dir = folder(&format!("customization-{profile}"));
        let config = if profile == "html" {
            std::fs::write(dir.join("source/letter.css"), ".flm-letter { color: teal; } /* File customization marker */\n").unwrap();
            json!({"custom_css_file": "./letter.css", "custom_css": ".flm-letter { color: maroon; } /* Inline customization marker </style><script>bad()</script> */"})
        } else {
            std::fs::write(dir.join("source/letter.tex"), "\\newcommand{\\LetterCustomizationMarker}{File customization marker}\n").unwrap();
            json!({"latex_preamble_file": "./letter.tex", "latex_preamble": "\\renewcommand{\\LetterCustomizationMarker}{Inline customization marker}"})
        };
        clean(&write_and_render(&dir, profile, config, BARE));
        let result = entry(&dir, profile);
        if profile == "html" {
            let stylesheet = std::fs::read_dir(dir.join("out")).unwrap().map(|entry| entry.unwrap().path())
                .find(|path| path.extension().is_some_and(|extension| extension == "css")
                    && std::fs::read_to_string(path).unwrap().contains("File customization marker")).unwrap();
            let css = std::fs::read_to_string(&stylesheet).unwrap();
            assert!(css.find("File customization marker").unwrap() < css.find("Inline customization marker").unwrap());
            assert!(css.contains("</style><script>bad()</script>"));
            assert!(!result.contains("Inline customization marker"), "custom CSS stays in its stylesheet");
            let href = format!("href=\"{}\"", stylesheet.file_name().unwrap().to_str().unwrap());
            assert!(result.find("font-family:").unwrap() < result.rfind("</style>").unwrap());
            assert!(result.rfind("</style>").unwrap() < result.find(&href).unwrap());
        } else {
            let file = result.find("File customization marker").unwrap();
            let inline = result.find("Inline customization marker").unwrap();
            assert!(file < inline);
            assert!(result.find("\\flmxLetterHeader").unwrap() < file);
            assert!(inline < result.find("\\begin{document}").unwrap());
        }
    }
}

#[test]
fn missing_resources_and_unsupported_logo_formats_report_errors() {
    for (index, (profile, config)) in [
        ("html", json!({"logo": "./missing.svg"})),
        ("html", json!({"custom_css_file": "./missing.css"})),
        ("latex-rendered", json!({"latex_preamble_file": "./missing.tex"})),
        ("html", json!({"logo": "./logo.txt"})),
        ("latex-rendered", json!({"logo": "./logo.txt"})),
        ("html", json!({"logo": "./logo.pdf"})),
        ("latex-rendered", json!({"logo": "./logo.svg"})),
        ("latex-rendered", json!({"logo": "./logo.gif"})),
        ("latex-rendered", json!({"logo": "./logo.webp"})),
    ].into_iter().enumerate() {
        let dir = folder(&format!("missing-resource-{index}"));
        // These files exist so rejection cannot be due to a missing resource.
        for extension in ["txt", "pdf", "svg", "gif", "webp"] {
            std::fs::write(dir.join(format!("source/logo.{extension}")), "Unsupported format for this output.").unwrap();
        }
        let output = write_and_render(&dir, profile, config, BARE);
        assert!(!output.status.success());
        assert!(common::diagnostics(&output).0.contains(&common::reported("error", "flm.compose.template-failed")), "{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn examples_render_and_optionally_compile_with_lualatex() {
    let compile = !matches!(std::env::var("FLM_TESTS_LATEX_COMPILE").unwrap_or_default().trim(), "" | "0");
    for name in ["europe", "us", "page-width"] {
        for profile in ["html", "latex-rendered"] {
            let dir = folder(&format!("example-{name}-{profile}"));
            let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/letter").join(format!("{name}.flm"));
            std::fs::copy(sample, dir.join("source/letter.flm")).unwrap();
            clean(&render(&dir, profile));
            if profile == "latex-rendered" && compile {
                let output = Command::new("lualatex").args(["-interaction=nonstopmode", "-halt-on-error", "letter.tex"])
                    .current_dir(dir.join("out")).output().unwrap();
                assert!(output.status.success(), "{name}: {}", String::from_utf8_lossy(&output.stdout));
                assert!(!String::from_utf8_lossy(&output.stdout).contains("Overfull"), "{name}: {}", String::from_utf8_lossy(&output.stdout));
            }
        }
    }
}
