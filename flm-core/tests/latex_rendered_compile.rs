//! Compiles every golden of the `latex-rendered` backend with `lualatex` and
//! checks the log.
//!
//! A golden under `tests/goldens/latex-rendered/` holds a complete document,
//! the backend's fixed skeleton, and the style file it loads, each under a
//! marker line. This suite writes both back out (the style file under its own
//! name), writes a one-pixel picture at the path of every image the document
//! shows, runs `lualatex` twice (the second run reads the labels the first
//! wrote), and asserts that the compilation succeeded and that the log holds
//! no undefined reference and no multiply-defined label.
//!
//! A second test compiles a document of its own over the style file of the
//! `headings.tex` golden, and checks that a heading leaves the paragraph after
//! it as LaTeX's own headings do: the same space around it, the same
//! indentation, a run-in title set once, and none of the heading's settings
//! left outside the scope it is written in.
//!
//! **The suite is skipped unless the environment variable
//! `FLM_TESTS_LATEX_COMPILE` is set** (to anything but the empty string or
//! `0`), with a printed message. Where it is set, a `lualatex` that cannot be
//! run from the `PATH` is a failure. Run it with
//!
//! ```text
//! FLM_TESTS_LATEX_COMPILE=1 cargo test -p flm-core --test latex_rendered_compile
//! ```
//!
//! The floor is a LaTeX system of 2022 or later with LuaLaTeX, `fontspec`,
//! `amsmath`, `xcolor`, `hyperref`, `graphicx` and `tabularray`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The marker line the document follows.
const DOCUMENT_MARKER: &str = "% ===== latex-rendered:document =====";
/// The prefix of the marker line the style file follows; the rest of the line
/// is the style file's name and ` =====`.
const STYLE_MARKER: &str = "% ===== latex-rendered:style ";

/// The environment variable that turns this suite on.
const ENABLE_VARIABLE: &str = "FLM_TESTS_LATEX_COMPILE";

/// A count making each compilation's directory unique.
static NEXT_COMPILE: AtomicUsize = AtomicUsize::new(0);

/// The directory of the backend's goldens.
fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/latex-rendered")
}

/// Whether the suite is on; where it is, `lualatex` must run.
fn enabled() -> bool {
    let value = std::env::var(ENABLE_VARIABLE).unwrap_or_default();
    if matches!(value.trim(), "" | "0") {
        return false;
    }
    let runs = Command::new("lualatex").arg("--version").output().is_ok_and(|out| out.status.success());
    assert!(runs, "{ENABLE_VARIABLE} is set, but `lualatex` cannot be run from the PATH");
    true
}

/// One golden split into the document and the style file with its name.
struct Golden {
    document: String,
    style: String,
    style_file: String,
}

/// Splits a golden's text at its two marker lines.
fn split(name: &str, text: &str) -> Golden {
    let rest = text
        .strip_prefix(DOCUMENT_MARKER)
        .and_then(|rest| rest.strip_prefix('\n'))
        .unwrap_or_else(|| panic!("`{name}` begins with the document marker"));
    let at = rest.find(STYLE_MARKER).unwrap_or_else(|| panic!("`{name}` holds the style marker"));
    let document = String::from(&rest[..at]);
    let after = &rest[at + STYLE_MARKER.len()..];
    let (line, style) = after.split_once('\n').unwrap_or_else(|| panic!("`{name}`: a style line"));
    let style_file = line.strip_suffix(" =====").unwrap_or(line).to_string();
    Golden { document, style: String::from(style), style_file }
}

/// A picture of one gray pixel, in PNG.
const ONE_PIXEL_PNG: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x7e, 0x9b,
    0x55, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x68, 0x00, 0x00, 0x00,
    0x82, 0x00, 0x81, 0x77, 0xcd, 0x72, 0xb6, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Writes [`ONE_PIXEL_PNG`] into `dir` at the path of every
/// `\flmxImage[…]{<path>}` of `document` whose path is a plain relative one.
fn write_images(dir: &Path, document: &str) {
    for (at, _) in document.match_indices("\\flmxImage") {
        let rest = &document[at + "\\flmxImage".len()..];
        let rest = match rest.strip_prefix('[') {
            Some(keys) => keys.split_once("]{").map_or("", |(_, after)| after),
            None => rest.strip_prefix('{').unwrap_or(""),
        };
        let Some((path, _)) = rest.split_once('}') else { continue };
        let plain = path.chars().all(|ch| ch.is_ascii_alphanumeric() || "/.-_".contains(ch));
        if path.is_empty() || !plain || path.split('/').any(|part| part == "..") {
            continue;
        }
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).expect("the image's folder");
        }
        fs::write(file, ONE_PIXEL_PNG).expect("the image");
    }
}

/// What a compilation left: whether both runs succeeded, and the log.
struct Compiled {
    ok: bool,
    log: String,
}

/// Writes `golden` out and runs `lualatex` twice over it.
fn compile(name: &str, golden: &Golden) -> Compiled {
    let unique = NEXT_COMPILE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "flm-latex-rendered-{}-{unique}-{}",
        std::process::id(),
        name.trim_end_matches(".tex")
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a temporary directory");
    fs::write(dir.join(&golden.style_file), &golden.style).expect("the style file");
    fs::write(dir.join("doc.tex"), &golden.document).expect("the document");
    write_images(&dir, &golden.document);
    let mut ok = true;
    for _ in 0..2 {
        let status = Command::new("lualatex")
            .current_dir(&dir)
            .args(["-interaction=nonstopmode", "-halt-on-error", "doc.tex"])
            .output()
            .unwrap_or_else(|error| panic!("lualatex runs: {error}"));
        if !status.status.success() {
            ok = false;
            break;
        }
    }
    let log = fs::read_to_string(dir.join("doc.log")).unwrap_or_default();
    let _ = fs::remove_dir_all(&dir);
    Compiled { ok, log }
}

/// The first line of a log that reports an error.
fn first_error(log: &str) -> String {
    log.lines()
        .find(|line| line.starts_with('!'))
        .map(String::from)
        .unwrap_or_else(|| String::from("(no `!` line in the log)"))
}

#[test]
fn every_golden_compiles_with_lualatex() {
    if !enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    let mut names: Vec<String> = fs::read_dir(goldens())
        .expect("the goldens' directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tex"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no golden to compile");
    let mut failures = Vec::new();
    for name in &names {
        let text = fs::read_to_string(goldens().join(name)).expect("the golden");
        let golden = split(name, &text);
        let compiled = compile(name, &golden);
        if !compiled.ok {
            failures.push(format!("{name}: {}", first_error(&compiled.log)));
            continue;
        }
        for warning in ["undefined references", "multiply defined", "Undefined control sequence"] {
            if compiled.log.contains(warning) {
                failures.push(format!("{name}: the log reports {warning}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Headings must leave the same paragraph continuation as LaTeX's native
/// section commands. Compilation alone misses lost spacing and indentation.
#[test]
fn headings_preserve_spacing_and_paragraph_state() {
    if !enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    // The style file the backend writes, as the headings golden holds it.
    let text = fs::read_to_string(goldens().join("headings.tex")).expect("the golden");
    let mut golden = split("headings.tex", &text);
    let package = golden.style_file.trim_end_matches(".sty");
    golden.document = format!(
        "\\documentclass{{article}}\n\\usepackage{{{package}}}\n{}",
        r"\makeatletter
\newbox\Expected
\newbox\Actual
\newcommand\Measure[2]{%
  \setbox#1\vbox{%
    \hsize=200pt \parindent=20pt \parskip=0pt
    Leading paragraph.\par
    #2{First heading}
    A paragraph after the first heading.\par
    #2{Second heading}
    A paragraph after the second heading.\par
  }%
}
\newcommand\Compare[2]{%
  \Measure\Expected{#1}%
  \Measure\Actual{#2}%
  \ifdim\ht\Actual=\ht\Expected\else
    \errmessage{Heading spacing differs from native LaTeX}%
  \fi
  \ifdim\dp\Actual=\dp\Expected\else
    \errmessage{Heading depth differs from native LaTeX}%
  \fi
}
\newcommand\FLMSection[1]{\flmxHeading[level=1,plain={#1}]{#1}}
\newcommand\FLMParagraph[1]{\flmxHeading[level=4,plain={#1}]{#1}}
\newcommand\FLMSubparagraph[1]{\flmxHeading[level=5,plain={#1}]{#1}}
\newcommand\CustomHeading[1]{%
  \@startsection{flmxheading}{1}{\z@}{40pt}{10pt}%
    {\normalfont\Large\bfseries}*{#1}%
}
\flmxDefineHeading{custom}{\CustomHeading{#1}}
\flmxDefineHeading{plain}{\par\large\bfseries#1\par}
\newcommand\FLMCustom[1]{\flmxHeading[kind=custom,level=1]{#1}}
\newcount\TitleCalls
\protected\def\CountTitle{\global\advance\TitleCalls by 1 }
\begin{document}
\flmxHeading[kind=plain]{A heading without sectioning machinery}
\Compare{\section*}{\FLMSection}
\Compare{\paragraph*}{\FLMParagraph}
\Compare{\subparagraph*}{\FLMSubparagraph}
\Compare{\CustomHeading}{\FLMCustom}

\FLMSection{No indent}
\if@afterindent\errmessage{Negative before-skip must suppress indentation}\fi
\if@nobreak\else\errmessage{A heading must stay with its first paragraph}\fi
\flmxSpan[style=emph]{First} paragraph.\par
\if@nobreak\errmessage{First paragraph must clear the heading no-break state}\fi

\FLMCustom{With indent}
\if@afterindent\else\errmessage{Positive before-skip must retain indentation}\fi
First paragraph.\par
\if@nobreak\errmessage{Custom heading left the no-break state set}\fi

% Adjacent headings intentionally suppress the second heading's before-skip.
\setbox\Expected\vbox{\section*{First}\subsection*{Second}Text.\par}
\setbox\Actual\vbox{%
  \flmxHeading[level=1]{First}\flmxHeading[level=2]{Second}Text.\par}
\ifdim\ht\Actual=\ht\Expected\else
  \errmessage{Consecutive heading spacing differs from native LaTeX}%
\fi

% Deferred titles survive the dispatcher group, and are inserted just once.
\flmxHeading[level=4,plain={Run in}]{\CountTitle Run in}
\ifnum\TitleCalls=0\else\errmessage{Run-in title was not deferred}\fi
\flmxSpan[style=emph]{First} paragraph.\par
\ifnum\TitleCalls=1\else\errmessage{Run-in title was lost or repeated}\fi
Second paragraph.\par
\ifnum\TitleCalls=1\else\errmessage{Run-in title was repeated}\fi
\if@noskipsec\errmessage{Run-in heading left a pending title}\fi

% Export into the containing scope only: no key/font leak, and no global
% paragraph-hook, deferred-title or label-name assignment across environments.
\begingroup
  \csname __flmx_set_keys:n\endcsname{kind=outer}
  \def\@currentlabelname{Outer name}%
  \let\OuterName\@currentlabelname
  \def\@svsechd{Outer deferred title}%
  \let\OuterTitle\@svsechd
  \everypar{\relax}%
  \edef\OuterEverypar{\the\everypar}%
  \@afterindenttrue
  \edef\OuterFont{\fontname\font}%
  \begingroup
    \flmxHeading[kind=section,level=1]{Inner heading}
    \edef\ActualKind{\flmxKey{kind}}%
    \def\ExpectedKind{outer}%
    \ifx\ActualKind\ExpectedKind\else\errmessage{Heading keys leaked}\fi
    \flmxIfKey{level}{\errmessage{Heading level leaked}}{}%
    \edef\ActualFont{\fontname\font}%
    \ifx\ActualFont\OuterFont\else\errmessage{Heading font leaked}\fi
    Text.\par
    \FLMParagraph{Inner run in}Text.\par
  \endgroup
  \edef\ActualEverypar{\the\everypar}%
  \ifx\ActualEverypar\OuterEverypar\else\errmessage{Paragraph hook leaked globally}\fi
  \ifx\@svsechd\OuterTitle\else\errmessage{Deferred title leaked globally}\fi
  \ifx\@currentlabelname\OuterName\else\errmessage{Heading name leaked globally}\fi
  \if@afterindent\else\errmessage{Indentation setting leaked globally}\fi
\endgroup
\end{document}
",
    );
    let compiled = compile("heading-paragraph-state", &golden);
    assert!(compiled.ok, "{}\n{}", first_error(&compiled.log), compiled.log);
}
