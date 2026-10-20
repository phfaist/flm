# syntax-highlight

Syntax highlighting for `latex-rendered` and `html`, written against `flm_ext::guest`.
Load `syntax-highlight` in `flm.use` or with `--use syntax-highlight`. The
extension adds lowering preferences to the existing `latex-rendered` and `html`
workflows; workflows that extend them (including `pdf-latex`) inherit those
preferences.
The playground's `moretemplates` package loads it automatically.

Use FLM's existing language arguments:

```latex
\verbcode[python]|print("Hello")|

\begin{verbatimcode}[rust]
fn main() {
    println!("Hello");
}
\end{verbatimcode}
```

Language names and file extensions are resolved by
[Syntect's `find_syntax_by_token`](https://docs.rs/syntect/latest/syntect/parsing/struct.SyntaxSet.html#method.find_syntax_by_token).
The bundled syntax set defines the supported languages; this extension adds no
aliases or custom grammars. Unknown or absent language tags remain plain code.
`verbtext` and identifiers (`verba`) remain plain as well.

The default theme is `InspiredGitHub`. Select another bundled Syntect theme with
`-O 'syntax-highlight.theme=Solarized (light)'`. The template still controls the
code font, block margins and background through the standard `flmxVerbatim`
hooks for LaTeX and the standard verbatim CSS properties for HTML.
Theme background colors are not applied.
The generated preamble defines `\flmxSyntaxToken{HTML color}{font switches}{text}`
for templates that want to customize token styling.

HTML keeps the standard `flm-pres-verbatim` class, kind, display and language
attributes, with `<code>` inline and `<pre><code>` for blocks. Highlighted tokens
are `<span class="flm-syntax-token">` elements with inline foreground color,
bold, italic and underline styles from the selected theme. Code without a known
language keeps the same verbatim markup without token spans. The HTML emitter
escapes all source text; highlighting adds no scripts or browser dependencies.

For an HTML template with light and dark appearances, set
`flm.features.syntax-highlight.html_dark_theme: base16-ocean.dark` (or another
bundled theme). Tokens then include CSS `light-dark()` colors: `theme` supplies
the light palette and `html_dark_theme` the dark palette. The containing code
element's CSS `color-scheme` selects the palette, so a template can switch
code backgrounds and foregrounds together. Font styles come from `theme`.
Browsers without `light-dark()` support use the primary foreground color.
The default empty `html_dark_theme` keeps the selected fixed palette. This
setting does not change LaTeX output. The `moretemplates` package enables the
paired HTML palette by default.

The lowering rule consumes `flm.pres.verbatim`: FLM has already determined the
kind, inline/block display, language and literal content. Syntect handles syntax
and multiline state for both backends; FLM's `escverb` arguments handle LaTeX
escaping, and HTML text nodes handle HTML escaping. No shell
escape, Python, or LaTeX-side source reconstruction is needed.

The extension is compiled into the CLI, like `profile-pdf-latex`. To build its standalone
WebAssembly form:

```sh
extensions/rust/build-wasm.sh syntax-highlight
```

The pure Rust regular-expression backend works in both forms.
