//! The style file of the `latex-rendered` backend: the per-document `.sty`
//! file the rendered LaTeX loads, assembled by [`style_body`] and
//! [`assemble`].
//!
//! The file holds, in order, each block under a comment line naming it:
//!
//! 1. the header, `\NeedsTeXFormat{LaTeX2e}[2022/06/01]` and
//!    `\ProvidesPackage{<name>}` and a comment line describing the file
//!    ([`assemble`]);
//! 2. the engine check, `\RequirePackage{iftex}` and `\RequireLuaTeX`, which
//!    stops a compilation under any engine other than LuaLaTeX with iftex's
//!    own message;
//! 3. the author's text written before the packages
//!    ([`ExtraPreamble::before_packages`]);
//! 4. the package block: one `\RequirePackage[<options>]{<package>}` line
//!    per package, LuaLaTeX's line of the merged requests
//!    ([`PackageRequest::requirepackage_line`]), each under a comment naming
//!    the identifiers behind it;
//! 5. `hyperref`, loaded at the end of the preamble through the kernel hook
//!    `begindocument/before`, so that a template's own packages load before
//!    it, with the options [`HYPERREF_OPTIONS`];
//! 6. the author's text written after the packages
//!    ([`ExtraPreamble::after_packages`]);
//! 7. the definition pieces: the core chunk, then every other piece the
//!    document requires, each under a comment line naming its identifier;
//! 8. the definitions rules staged as definition constructs, once by name;
//! 9. the author's text written at the end ([`ExtraPreamble::at_end`]);
//! 10. `\endinput`.
//!
//! No engine test is written anywhere else in the file. The file's name is the
//! render option `style_stem` and eight hexadecimal digits of the digest of
//! everything but the header, so that two documents needing the same file share
//! one name.

use crate::stdcore::backends::latex::{ExtraPreamble, LatexPreambleDef, PackageRequest};
use crate::untechxt::preamble::Engine;

use super::emit::StagedDefinition;

/// The options the style file passes `hyperref`: colored links in the three
/// colors of the core chunk, which a template redefines.
pub const HYPERREF_OPTIONS: &str =
    "colorlinks=true, linkcolor=flmxLinkColor, urlcolor=flmxUrlColor, citecolor=flmxCiteColor";

/// The LaTeX format the style file needs, a LaTeX of 2022 or later.
pub const FORMAT_DATE: &str = "2022/06/01";

/// What the style file holds besides its header.
#[derive(Debug, Clone, Copy)]
pub struct StyleParts<'a> {
    /// The merged package requests, in the order first met.
    pub packages: &'a [PackageRequest],
    /// Whether `hyperref` is loaded at the end of the preamble.
    pub hyperref: bool,
    /// The definition pieces, the core chunk first.
    pub pieces: &'a [&'a LatexPreambleDef],
    /// The definitions rules staged, each once by name.
    pub definitions: &'a [StagedDefinition],
    /// The author's own text.
    pub extra: &'a ExtraPreamble,
}

/// Appends `text`, ending it with a newline where it has none.
fn push_lines(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str(text);
    if !text.ends_with('\n') {
        out.push('\n');
    }
}

/// The style file without its header: items 2 to 10 of the [module
/// documentation](self).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::ExtraPreamble;
/// use flm_core::stdcore::backends::latex_rendered::style::{StyleParts, style_body};
///
/// let extra = ExtraPreamble::default();
/// let body = style_body(&StyleParts {
///     packages: &[],
///     hyperref: true,
///     pieces: &[],
///     definitions: &[],
///     extra: &extra,
/// });
/// assert!(body.starts_with("% the engine: LuaLaTeX\n\\RequirePackage{iftex}\n\\RequireLuaTeX\n"));
/// assert!(body.contains("\\AddToHook{begindocument/before}{\\RequirePackage[colorlinks=true"));
/// assert!(body.ends_with("\\endinput\n"));
/// ```
pub fn style_body(parts: &StyleParts<'_>) -> String {
    let mut out = String::new();
    out.push_str("% the engine: LuaLaTeX\n\\RequirePackage{iftex}\n\\RequireLuaTeX\n");
    if !parts.extra.before_packages.is_empty() {
        out.push_str("% the author's text before the packages\n");
        push_lines(&mut out, &parts.extra.before_packages);
    }
    out.push_str("% the packages\n");
    for request in parts.packages {
        let Some(line) = request.requirepackage_line(Engine::LuaLatex) else { continue };
        let ids: Vec<&str> = request
            .for_engine(Engine::LuaLatex)
            .map(|line| line.ids.iter().map(|id| id.as_str()).collect())
            .unwrap_or_default();
        out.push_str(&format!("% {}\n{line}\n", ids.join(", ")));
    }
    if parts.hyperref {
        out.push_str("% hyperref, loaded at the end of the preamble\n");
        out.push_str(&format!(
            "\\AddToHook{{begindocument/before}}{{\\RequirePackage[{HYPERREF_OPTIONS}]{{hyperref}}}}\n"
        ));
    }
    if !parts.extra.after_packages.is_empty() {
        out.push_str("% the author's text after the packages\n");
        push_lines(&mut out, &parts.extra.after_packages);
    }
    for piece in parts.pieces {
        out.push_str(&format!("% {}\n", piece.id));
        push_lines(&mut out, &piece.content);
    }
    if !parts.definitions.is_empty() {
        out.push_str("% the definitions the document's rules staged\n");
        for definition in parts.definitions {
            push_lines(&mut out, &definition.text);
        }
    }
    if !parts.extra.at_end.is_empty() {
        out.push_str("% the author's text at the end\n");
        push_lines(&mut out, &parts.extra.at_end);
    }
    out.push_str("\\endinput\n");
    out
}

/// The whole style file whose LaTeX package name is `name` (the file's name
/// without `.sty`): the header, then `body` ([`style_body`]).
///
/// `\ProvidesPackage` takes no optional argument: LaTeX reads the start of
/// that text as the package's date, and a text not beginning with a date stops
/// the compilation at `\begin{document}`. The description is the comment line
/// that follows it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::style::assemble;
///
/// let file = assemble("job-flmx.1a2b3c4d", "\\endinput\n");
/// assert_eq!(
///     file,
///     "\\NeedsTeXFormat{LaTeX2e}[2022/06/01]\n\
///      \\ProvidesPackage{job-flmx.1a2b3c4d}\n\
///      % the definitions of a document written by FLM's latex-rendered backend\n\
///      \\endinput\n"
/// );
/// ```
pub fn assemble(name: &str, body: &str) -> String {
    format!(
        "\\NeedsTeXFormat{{LaTeX2e}}[{FORMAT_DATE}]\n\\ProvidesPackage{{{name}}}\n\
         % the definitions of a document written by FLM's latex-rendered backend\n{body}"
    )
}
