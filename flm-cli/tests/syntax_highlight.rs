//! The guest extension consumes resolved verbatim payloads for LaTeX and HTML.
#![cfg(feature = "bundled-extensions")]
use std::path::Path;
use std::process::Command;

#[test]
fn language_tagged_code_is_highlighted_and_plain_code_stays_plain() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("syntax-highlight");
    if dir.exists() { std::fs::remove_dir_all(&dir).unwrap(); }
    std::fs::create_dir_all(&dir).unwrap();
    let document = r#"---
flm:
  use: [syntax-highlight]
---
Inline: \verbcode[python]|print("café")|.

\begin{verbatimcode}[rust]
fn main() {
    println!("{} % _ \\ café", 42);
}
\end{verbatimcode}

\begin{verbatimcode}[no-such-language]
plain & < > # $ { } _ % \

last
\end{verbatimcode}

\verbtext|plain & %|
"#;
    std::fs::write(dir.join("code.flm"), document).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .current_dir(&dir).args(["render", "code.flm", "-p", "latex-rendered", "-o", "code.tex"])
        .output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let tex = std::fs::read_to_string(dir.join("code.tex")).unwrap();
    let sty = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sty"))
        .map(|path| std::fs::read_to_string(path).unwrap()).collect::<String>();
    assert!(tex.contains("\\flmxSyntaxToken"), "{tex}");
    assert!(tex.contains("lang={python}") && tex.contains("lang={rust}"), "{tex}");
    assert!(tex.contains("\\flmxSyntaxPlain{plain\\ \\&"), "{tex}");
    assert!(tex.contains("\\%") && tex.contains("\\_") && tex.contains("\\textbackslash{}"), "{tex}");
    assert!(tex.contains("café"), "{tex}");
    assert!(sty.contains("\\newcommand\\flmxSyntaxToken"), "{sty}");

    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .current_dir(&dir).args(["render", "code.flm", "-p", "html", "-o", "code.html"])
        .output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!std::fs::read_to_string(dir.join("code.html")).unwrap().contains("flmxSyntaxToken"));
}

#[test]
fn html_reuses_highlighting_and_preserves_verbatim_content_and_layout() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("syntax-highlight-html");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let document = r#"---
flm:
  use: [syntax-highlight]
---
Inline: \verbcode[python]|print("café & <script>")|.

\begin{verbatimcode}[python]
x = """first
  second"""
# <script>alert("&")</script> café

	print(x)
\end{verbatimcode}

\begin{verbatimcode}[no-such-language]
plain & < > # $ { } _ % \

last
\end{verbatimcode}

\verbcode|unlabelled < & >| \verbtext|plain & %| \verba|identifier|

\verbcode[python]||
"#;
    std::fs::write(dir.join("code.flm"), document).unwrap();
    let render = |file: &str, options: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_flm"))
            .current_dir(&dir)
            .args(["render", file, "-p", "html", "-o", "code.html"])
            .args(options)
            .output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        std::fs::read_to_string(dir.join("code.html")).unwrap()
    };
    let html = render("code.flm", &[]);
    let code = regex::Regex::new(r"(?s)<code(?: [^>]*)?>(.*?)</code>").unwrap();
    let bodies = |html: &str| code.captures_iter(html).map(|c| c[1].to_owned()).collect::<Vec<_>>();
    let highlighted = bodies(&html);
    assert_eq!(highlighted.len(), 7, "{html}");
    for body in &highlighted[..2] {
        assert!(body.contains("class=\"flm-syntax-token\" style=\"color: #"), "{body}");
    }
    for body in &highlighted[2..] {
        assert!(!body.contains("<span"), "{body}");
    }
    assert!(html.contains("<pre class=\"flm-pres-verbatim\" data-flm-kind=\"code\" data-flm-display=\"block\" data-flm-lang=\"python\"><code>"), "{html}");
    assert!(html.contains("--flm-verbatim-family:"), "verbatim stylesheet is required");
    assert!(html.contains("&lt;script&gt;") && !html.contains("<script>alert"), "{html}");

    // Removing just the generated token tags must recover the default backend's
    // escaped content, including newlines, tabs, blank lines and Unicode.
    std::fs::write(dir.join("plain.flm"), document.replace("use: [syntax-highlight]", "use: []")).unwrap();
    let plain = render("plain.flm", &[]);
    let tokens = regex::Regex::new(r#"<span class="flm-syntax-token" style="[^"]*">|</span>"#).unwrap();
    let unstyled: Vec<_> = highlighted.iter().map(|body| tokens.replace_all(body, "").into_owned()).collect();
    assert_eq!(unstyled, bodies(&plain));

    let themed = render("code.flm", &["-O", "syntax-highlight.theme=Solarized (light)"]);
    assert_ne!(bodies(&themed)[0], highlighted[0], "the selected theme affects HTML tokens");
    let themed_unstyled: Vec<_> = bodies(&themed).iter().map(|body| tokens.replace_all(body, "").into_owned()).collect();
    assert_eq!(themed_unstyled, unstyled);

    let adaptive = render("code.flm", &["-O", "syntax-highlight.html_dark_theme=base16-ocean.dark"]);
    assert!(adaptive.contains("color: light-dark(#"), "{adaptive}");
    let adaptive_unstyled: Vec<_> = bodies(&adaptive).iter().map(|body| tokens.replace_all(body, "").into_owned()).collect();
    assert_eq!(adaptive_unstyled, unstyled, "paired palettes preserve all source text");
}
