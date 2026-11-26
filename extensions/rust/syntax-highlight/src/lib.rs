//! Syntax highlighting of FLM's resolved verbatim content, using Syntect.
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, Staged,
};
use flm_ext::guest::core::registry::{Contributed, FeatureBuilder};
use flm_ext::guest::lang::text::chars_text;
use flm_ext::guest::stdlib::backends::{html, latex, pres};
use flm_ext::guest::techy::node::Node;
use flm_ext::{
    ExtError,
    guest::{Extension, Feature, Package},
};
use serde_json::{Value, json};
use std::sync::{Arc, OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style, Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

const RULE: &str = "syntax-highlight.latex";
const HTML_RULE: &str = "syntax-highlight.html";
const PIECE: &str = "flm.latex.def.syntax-highlight.tokens";

/// The extension's code, which its row of the table of code of `flm-extensions` holds: the
/// function that declares its packages, and the extension's name and version.
pub static FLM_EXTENSION: flm_ext::guest::CompiledExtension =
    flm_ext::guest::CompiledExtension::new(extension, "syntax-highlight", env!("CARGO_PKG_VERSION"));

fn extension(ext: &mut Extension) {
    ext.package("_", Package::new().feature("syntax-highlight", Feature::new().title("Syntax highlighting")
        .docs("Highlights language-labelled verbatim code with Syntect's bundled syntaxes and themes.")
        .settings(json!({"config": {"schema": {"type": "object", "properties": {
            "theme": {"type": "string", "default": "InspiredGitHub", "description": "A theme from Syntect's bundled theme set."},
            "html_dark_theme": {"type": "string", "default": "", "description": "Optional HTML dark-mode theme from Syntect's bundled theme set. Empty keeps a fixed palette. Token colors follow the containing element's CSS color-scheme; font styles use the primary theme."}
        }}}}))
        .register(register)));
}

fn register(b: &mut FeatureBuilder, settings: &Value) -> Result<(), ExtError> {
    b.workflow_lowering(
        "latex-rendered",
        &json!({"latex-rendered": {"prefer": [RULE]}}),
    )?;
    b.workflow_lowering("html", &json!({"html": {"prefer": [HTML_RULE]}}))?;
    let name = settings["theme"].as_str().unwrap_or("InspiredGitHub");
    let theme = ThemeSet::load_defaults()
        .themes
        .remove(name)
        .ok_or_else(|| ExtError::Value {
            detail: format!("unknown Syntect theme `{name}`"),
        })?;
    let dark_name = settings["html_dark_theme"].as_str().unwrap_or("");
    let html_dark_theme = if dark_name.is_empty() {
        None
    } else {
        Some(ThemeSet::load_defaults().themes.remove(dark_name).ok_or_else(|| ExtError::Value {
            detail: format!("unknown Syntect theme `{dark_name}`"),
        })?)
    };
    for element in [
        pres::VERBATIM,
        latex::MACRO,
        latex::ENVIRONMENT,
        latex::RAW,
        html::TAG,
    ] {
        b.peer_element(
            element,
            "Verbatim input and backend output for syntax highlighting.",
        )?;
    }
    b.peer_backend("latex-rendered", "The LaTeX backend.")?;
    b.peer_backend("html", "The HTML backend.")?;
    b.peer_slot(latex::preamble::SLOT, Contributed::Data(json!({
        "name": "syntax-highlight.tokens",
        "content": "\\newcommand\\flmxSyntaxToken[3]{{\\color[HTML]{#1}#2#3}}\n\\newcommand\\flmxSyntaxPlain[1]{#1}\n",
        "requires": ["flm.latex.def.flmx.verbatim", "flm.latex.pkg.xcolor"]
    })))?;
    b.lowering_rule(LoweringRule::new(
        RULE,
        pres::VERBATIM,
        LoweringDecl::new()
            .produces(latex::MACRO)
            .produces(latex::ENVIRONMENT)
            .produces(latex::RAW)
            .selection_requires(PIECE),
        Arc::new(Highlight {
            theme: theme.clone(),
            html_dark_theme: None,
            html: false,
        }),
        DefaultMark::OnBackends(vec!["latex-rendered".into()]),
    )?)?;
    b.lowering_rule(LoweringRule::new(
        HTML_RULE,
        pres::VERBATIM,
        LoweringDecl::new().produces(html::TAG),
        Arc::new(Highlight { theme, html_dark_theme, html: true }),
        DefaultMark::OnBackends(vec!["html".into()]),
    )?)
}

struct Highlight {
    theme: Theme,
    html_dark_theme: Option<Theme>,
    html: bool,
}

fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The spans of one highlighted line: each a piece of the line's source text with its
/// style, or without one where no syntax applies.
type LineSpans<'a> = Vec<(Option<Style>, &'a str)>;

/// Each line keeps its source text exactly; styles refer to Syntect's own theme.
fn highlight<'a>(
    text: &'a str,
    language: Option<&str>,
    theme: &Theme,
) -> Result<Vec<LineSpans<'a>>, ExtError> {
    let ss = syntaxes();
    let mut highlighter = language
        .and_then(|lang| ss.find_syntax_by_token(lang))
        .map(|syntax| HighlightLines::new(syntax, theme));
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(text) {
        let spans = match &mut highlighter {
            Some(h) => h
                .highlight_line(line, ss)
                .map_err(|e| ExtError::Value {
                    detail: format!("syntax highlighting failed: {e}"),
                })?
                .into_iter()
                .map(|(style, text)| (Some(style), text))
                .collect(),
            None => vec![(None, line)],
        };
        lines.push(spans);
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    Ok(lines)
}

fn raw(cx: &mut LowerCx, lw: &mut Lowerer, text: &str) -> Result<Staged, ExtError> {
    lw.pres(cx, latex::RAW, &json!({"text": text}), vec![].into())
}

fn call(
    cx: &mut LowerCx,
    lw: &mut Lowerer,
    name: &str,
    args: Vec<(&str, Vec<Staged>)>,
) -> Result<Staged, ExtError> {
    let shapes: Vec<_> = args.iter().map(|(shape, _)| *shape).collect();
    let payload = json!({"name": name, "args": shapes});
    lw.pres_slots(
        cx,
        latex::MACRO,
        &payload,
        args.into_iter()
            .map(|(_, nodes)| (latex::ARG_SLOT.into(), nodes.into()))
            .collect(),
    )
}

impl Lower for Highlight {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let payload = node.payload()?.ok_or_else(|| ExtError::Value {
            detail: "verbatim payload is missing".into(),
        })?;
        let text =
            chars_text(&node.body()?.unwrap_or_default())?.ok_or_else(|| ExtError::Value {
                detail: "resolved verbatim content must be characters".into(),
            })?;
        // Only code (`\verbcode`, `{verbatimcode}`) is highlighted. Plain
        // characters and an identifier are not code: they are written as the
        // backend writes them without this extension.
        if payload["kind"] != "code" {
            return if self.html {
                plain_html(&payload, &text, cx, lw)
            } else {
                plain_latex(&payload, &text, cx, lw)
            };
        }
        let block = payload["display"] == "block";
        let language = payload["lang"].as_str();
        let lines = highlight(&text, language, &self.theme)?;
        if self.html {
            let dark_lines = self.html_dark_theme.as_ref()
                .map(|theme| highlight(&text, language, theme)).transpose()?;
            return highlighted_html(&payload, lines, dark_lines, cx, lw);
        }
        let mut body = Vec::new();
        for spans in lines {
            let mut content = Vec::new();
            for (style, source) in spans {
                let source = if block {
                    source.trim_end_matches('\n').to_owned()
                } else {
                    source.replace('\n', " ")
                };
                if source.is_empty() {
                    continue;
                }
                let text = lw.text(cx, &source)?;
                if let Some(style) = style {
                    let c = style.foreground;
                    let color = lw.text(cx, &format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b))?;
                    let mut flags = String::new();
                    if style.font_style.contains(FontStyle::BOLD) {
                        flags.push_str("\\bfseries ");
                    }
                    if style.font_style.contains(FontStyle::ITALIC) {
                        flags.push_str("\\itshape ");
                    }
                    let flags = raw(cx, lw, &flags)?;
                    let token = call(
                        cx,
                        lw,
                        "flmxSyntaxToken",
                        vec![
                            ("mandatory", vec![color]),
                            ("mandatory", vec![flags]),
                            ("escverb", vec![text]),
                        ],
                    )?;
                    content.push(if style.font_style.contains(FontStyle::UNDERLINE) {
                        call(cx, lw, "underline", vec![("mandatory", vec![token])])?
                    } else {
                        token
                    });
                } else {
                    // The backend's escverb argument performs all character escaping.
                    content.push(call(
                        cx,
                        lw,
                        "flmxSyntaxPlain",
                        vec![("escverb", vec![text])],
                    )?);
                }
            }
            if block {
                body.push(call(cx, lw, "flmxVerbLine", vec![("mandatory", content)])?);
            } else {
                body.extend(content);
            }
        }
        let mut keys = vec![
            raw(cx, lw, "kind={")?,
            lw.text(cx, payload["kind"].as_str().unwrap_or("text"))?,
            raw(cx, lw, "}")?,
        ];
        if let Some(lang) = payload["lang"].as_str() {
            keys.extend([
                raw(cx, lw, ",lang={")?,
                lw.text(cx, lang)?,
                raw(cx, lw, "}")?,
            ]);
        }
        let result = if block {
            lw.pres_slots(
                cx,
                latex::ENVIRONMENT,
                &json!({"name": "flmxVerbatimBlock", "args": ["optional"], "verbatim": false}),
                vec![(latex::ARG_SLOT.into(), keys.into()), ("body".into(), body.into())],
            )?
        } else {
            call(
                cx,
                lw,
                "flmxVerbatim",
                vec![("optional", keys), ("mandatory", body)],
            )?
        };
        Ok(Lowering::Emit(vec![result].into()))
    }
}

/// Intersect both themes' token boundaries: adjacent syntax scopes can have the
/// same style in one theme and different styles in the other. Both inputs cover
/// exactly the same source text, including Unicode and multiline state.
fn paired_spans<'a>(
    lines: Vec<LineSpans<'a>>,
    dark_lines: Option<Vec<LineSpans<'a>>>,
) -> Vec<(Option<Style>, Option<Style>, &'a str)> {
    let mut result = Vec::new();
    let Some(dark_lines) = dark_lines else {
        return lines.into_iter().flatten().map(|(style, text)| (style, None, text)).collect();
    };
    let mut dark = dark_lines.into_iter().flatten();
    let mut current = dark.next();
    for (style, mut text) in lines.into_iter().flatten() {
        while !text.is_empty() {
            let (dark_style, dark_text) = current.as_mut().expect("both themes cover the source");
            let length = text.len().min(dark_text.len());
            if length > 0 {
                result.push((style, *dark_style, &text[..length]));
                text = &text[length..];
                *dark_text = &dark_text[length..];
            }
            if dark_text.is_empty() {
                current = dark.next();
            }
        }
    }
    result
}

/// Keep the standard verbatim HTML contract, with escaped text inside token spans.
fn highlighted_html(
    payload: &Value,
    lines: Vec<LineSpans<'_>>,
    dark_lines: Option<Vec<LineSpans<'_>>>,
    cx: &mut LowerCx,
    lw: &mut Lowerer,
) -> Result<Lowering, ExtError> {
    let mut body = Vec::new();
    for (style, dark_style, source) in paired_spans(lines, dark_lines) {
        let text = lw.text(cx, source)?;
        body.push(if let Some(style) = style {
            let c = style.foreground;
            let mut css = format!("color: #{:02X}{:02X}{:02X};", c.r, c.g, c.b);
            if let Some(dark_style) = dark_style {
                let d = dark_style.foreground;
                css.push_str(&format!(" color: light-dark(#{:02X}{:02X}{:02X}, #{:02X}{:02X}{:02X});", c.r, c.g, c.b, d.r, d.g, d.b));
            }
            if style.font_style.contains(FontStyle::BOLD) {
                css.push_str(" font-weight: bold;");
            }
            if style.font_style.contains(FontStyle::ITALIC) {
                css.push_str(" font-style: italic;");
            }
            if style.font_style.contains(FontStyle::UNDERLINE) {
                css.push_str(" text-decoration: underline;");
            }
            lw.pres(
                cx,
                html::TAG,
                &json!({"tag": "span", "attributes": [["class", "flm-syntax-token"], ["style", css]]}),
                vec![text].into(),
            )?
        } else {
            text
        });
    }
    let block = payload["display"] == "block";
    if block {
        body = vec![lw.pres(cx, html::TAG, &json!({"tag": "code"}), body.into())?];
    }
    lw.require("flm.html.style.verbatim")?;
    let result = lw.pres(
        cx,
        html::TAG,
        &json!({"tag": if block { "pre" } else { "code" }, "attributes": html_attributes(payload), "block": block}),
        body.into(),
    )?;
    Ok(Lowering::Emit(vec![result].into()))
}

/// The standard verbatim class and attributes of the HTML contract: the kind, the
/// display and the language.
fn html_attributes(payload: &Value) -> Vec<Value> {
    let mut attributes = vec![
        json!(["class", "flm-pres-verbatim"]),
        json!(["data-flm-kind", payload["kind"]]),
        json!(["data-flm-display", payload["display"]]),
    ];
    if let Some(lang) = payload["lang"].as_str() {
        attributes.push(json!(["data-flm-lang", lang]));
    }
    attributes
}

/// The text nodes of `text`: none for an empty text.
fn text_nodes(cx: &mut LowerCx, lw: &mut Lowerer, text: &str) -> Result<Vec<Staged>, ExtError> {
    if text.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![lw.text(cx, text)?])
    }
}

/// Plain characters or an identifier, not highlighted, written as the HTML backend
/// writes them: `<span>` inline and `<div>` as a block, with the standard verbatim
/// class and attributes.
fn plain_html(
    payload: &Value,
    text: &str,
    cx: &mut LowerCx,
    lw: &mut Lowerer,
) -> Result<Lowering, ExtError> {
    let block = payload["display"] == "block";
    let body = text_nodes(cx, lw, text)?;
    lw.require("flm.html.style.verbatim")?;
    let result = lw.pres(
        cx,
        html::TAG,
        &json!({"tag": if block { "div" } else { "span" }, "attributes": html_attributes(payload), "block": block}),
        body.into(),
    )?;
    Ok(Lowering::Emit(vec![result].into()))
}

/// Plain characters or an identifier, not highlighted, written as the
/// `latex-rendered` backend writes them: `\flmxVerbatim[kind, lang]{…}` inline, a
/// newline in it written as a space; as a block, a `flmxVerbatimBlock` environment
/// holding one `\flmxVerbLine{…}` per line.
fn plain_latex(
    payload: &Value,
    text: &str,
    cx: &mut LowerCx,
    lw: &mut Lowerer,
) -> Result<Lowering, ExtError> {
    let mut keys = vec![raw(cx, lw, "kind=")?];
    keys.extend(name_value(cx, lw, payload["kind"].as_str().unwrap_or("text"))?);
    if let Some(lang) = payload["lang"].as_str() {
        keys.push(raw(cx, lw, ", lang=")?);
        keys.extend(name_value(cx, lw, lang)?);
    }
    let result = if payload["display"] == "block" {
        let text = text.strip_suffix('\n').unwrap_or(text);
        let mut lines = Vec::new();
        for line in text.split('\n') {
            let line = text_nodes(cx, lw, line)?;
            lines.push(call(cx, lw, "flmxVerbLine", vec![("escverb", line)])?);
        }
        lw.pres_slots(
            cx,
            latex::ENVIRONMENT,
            &json!({"name": "flmxVerbatimBlock", "args": ["optional"], "verbatim": false}),
            vec![(latex::ARG_SLOT.into(), keys.into()), ("body".into(), lines.into())],
        )?
    } else {
        let body = text_nodes(cx, lw, &text.replace('\n', " "))?;
        call(cx, lw, "flmxVerbatim", vec![("optional", keys), ("escverb", body)])?
    };
    Ok(Lowering::Emit(vec![result].into()))
}

/// A key's value as the `latex-rendered` backend writes a name: bare when it holds
/// only letters, digits and `.-_:/+@*`, in braces otherwise.
fn name_value(cx: &mut LowerCx, lw: &mut Lowerer, name: &str) -> Result<Vec<Staged>, ExtError> {
    if !name.is_empty()
        && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ".-_:/+@*".contains(ch))
    {
        Ok(vec![raw(cx, lw, name)?])
    } else {
        Ok(vec![raw(cx, lw, "{")?, lw.text(cx, name)?, raw(cx, lw, "}")?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighting_preserves_every_character_and_handles_multiline_strings() {
        let theme = ThemeSet::load_defaults()
            .themes
            .remove("InspiredGitHub")
            .unwrap();
        let source = "x = \"\"\"first\n  second\"\"\"\n# {\\}%_ café\n\tprint(x)\n\n";
        let lines = highlight(source, Some("python"), &theme).unwrap();
        assert_eq!(
            lines.iter().flatten().map(|(_, s)| *s).collect::<String>(),
            source
        );
        assert!(lines.iter().flatten().any(|(style, _)| style.is_some()));
        assert_eq!(lines[0].last().unwrap().0, lines[1].first().unwrap().0);
    }
    #[test]
    fn unsupported_and_unlabelled_code_is_plain() {
        let theme = ThemeSet::load_defaults()
            .themes
            .remove("InspiredGitHub")
            .unwrap();
        for language in [None, Some("no-such-language")] {
            let lines = highlight("a\n\n", language, &theme).unwrap();
            assert!(lines.iter().flatten().all(|(style, _)| style.is_none()));
            assert_eq!(lines.len(), 2);
        }
        assert_eq!(highlight("", None, &theme).unwrap().len(), 1);
    }

    #[test]
    fn paired_themes_preserve_source_and_both_palettes_at_every_boundary() {
        let themes = ThemeSet::load_defaults().themes;
        let primary = &themes["InspiredGitHub"];
        let source = "x = \"\"\"café 🦀\n  second\"\"\"\n# comment\n\tprint(x, 42)\n\n";
        for alternate in themes.values() {
            for language in [Some("python"), Some("rust"), Some("unknown"), None] {
                for source in [source, ""] {
                    let light = highlight(source, language, primary).unwrap();
                    let dark = highlight(source, language, alternate).unwrap();
                    let expected_light: Vec<_> = light.iter().flatten()
                        .flat_map(|(style, text)| text.chars().map(move |_| *style)).collect();
                    let expected_dark: Vec<_> = dark.iter().flatten()
                        .flat_map(|(style, text)| text.chars().map(move |_| *style)).collect();
                    let paired = paired_spans(light, Some(dark));
                    assert_eq!(paired.iter().map(|(_, _, text)| *text).collect::<String>(), source);
                    assert_eq!(paired.iter().flat_map(|(style, _, text)| text.chars().map(move |_| *style)).collect::<Vec<_>>(), expected_light);
                    assert_eq!(paired.iter().flat_map(|(_, style, text)| text.chars().map(move |_| *style)).collect::<Vec<_>>(), expected_dark);
                }
            }
        }
    }
}
