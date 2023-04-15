# A Flexible Latex-Like Markup (FLM) language

This package provides a simple parser and formatter for a custom markup language
that is inspired by LaTeX syntax.

The syntax of FLM is essentially a subset of standard LaTeX commands, including
macros, environments, and some characters that have a special meaning; these
features are parsed in a loosely similar fashion to usual LaTeX code.

The framework is meant to be very easily extendible and customizable.  The
parser is based on [*pylatexenc 3*](https://github.com/phfaist/pylatexenc)
(which is currently still in development).

FLM is used to write the contents of the [Error Correction
Zoo](https://errorcorrectionzoo.org/) in a way that is intuitive for scientists,
flexible, and robust.  It is easily extensible and closely resembles the LaTeX
langauge that many scientists are familiar with.  FLM pushes further one of the
core insights of LaTeX, namely, that the code should *describe* document
contents in an intuitive way, as a markup language, while disregarding as much
as possible the details of how that contents will be typeset.  The final
typesetting is fully customizable, e.g., through CSS styling of its HTML output
(including the use of templates).

You can install FLM with pip:
```bash
$ pip install flm-core
```

Example `mydocument.flm`:
```latex
---
title: Kitaev's Surface Code
---
\section{Kitaev's Surface Code}

The \emph{stabilizers} of the \textit{surface code} on the 2-dimensional
torus are generated by star operators \(A_v\) and plaquette operators
\(B_p\).  Each star operator is a \textbf{product} of four Pauli-\(X\)
operators on the edges adjacent to a vertex \(v\) of the lattice; each
plaquette operator is a product of four Pauli-\(Z\) operators applied to
the edges adjacent to a face, or plaquette, \(p\) of the lattice
(\ref{figure:toric-code-operators}).

\begin{figure}
  \includegraphics{toric-code-operators}
  \caption{Stabilizer generators and logical operators of the 2D surface
    code on a torus.  The star operators \(A_v\) and the plaquette
    operators \(B_p\) generate the stabilizer group of the toric code.
    The logical operators are strings that wrap around the torus.}
  \label{figure:toric-code-operators}
\end{figure}

...
```

To compile your document into an HTML page, use:
```bash
$ flm mydocument.flm -o mydocument.html --format=html --template=simple
```

You can then open the `mydocument.html` file in your browser.

# This is work in progress!

This project is still early in an active development stage, and there might
still be a few bugs around. You can expect the API to still change pretty
drastically.  Feel free to share ideas!

# Use as a command-line tool

You can use `flm` in command-line mode to compile your documents:
```bash
$ flm mydocument.flm
```

Run `flm --help` to get a list of options. They should be fairly
standard and/or self-explanatory:
```bash
# output to file mydocument.html, format HTML, including skeleton
# HTML structure with minimal CSS.
$ flm mydocument.flm -o mydocument.html --format=html --template=simple
```

Available formats are `html`, `text`, `latex`, and `markdown`.  Formats
`text`, `markdown`, and `latex` are very experimental!  (You can also
generate `pdf` output with the options `--workflow=runlatexpdf --format=pdf`
if your system has a standard LaTeX distribution such as TeXLive installed)

- **Additional HTML Templates:**
  The `--template=` option can be used to change the template used to render the
  document.  See also the
  [*flm-templates*](https://github.com/phfaist/flm-templates) extension package
  for some additional templates and template engines.  You can try:
  ```
  > pip install flm-templates flm-htmlplus
  ```
  and then
  ```
  > flm mydocument.flm -o output.html -w flm_htmlplus -P 'pkg:flm_templates' -t sunset
  ```
  Or try the template `-t oldtextbook`.

- **Citations from arXiv & DOI:**
  Automatically fetch citations from the arXiv, DOI x-references, or other
  sources using the [*flm-citations*](https://github.com/phfaist/flm-citations)
  extension package (see README file there).  Install the extension package
  using pip:
  ```
  > pip install git+https://github.com/phfaist/flm-citations
  ```
  And then try to compile, e.g., the following FLM document:
  ```yaml
  ---
  $import:
    - pkg:flm_citations
  ---
  \section{Introduction}
  Let's cite Kitaev's surface code~\cite{doi:10.1070/RM1997v052n06ABEH002155,doi:10.1007/978-1-4615-5923-8_19,arXiv:quant-ph/9707021}. ...
  ```

- **Theorems and proofs:** Use the environments
  ``\begin{theorem}..\end{theorem}``, ``{lemma}``, ``{proposition}``,
  ``{definition}``, ``{remark}``, etc. for typesetting theorems.  You can pin
  labels to theorem statements ``\label{thm:XYZ}`` (use the `thm:` or `x:`
  prefixes regardless of the theorem type, so you can easily change it without
  having to update the label).  These environments accept an optional argument
  so you can specify a title, e.g. ``\begin{theorem}[Euler's theorem]
  ... \end{theorem}``.  Typeset proofs with ``\begin{proof} ... \end{proof}``.
  The proof's optional argument enjoys an additional syntax: use
  ``\begin{proof}[*thm:XYZ] ...`` to typeset `Proof (Theorem 3)` (with the
  correct theorem reference using its label, which must match the one specified
  to the theorem's ``\label`` command).  Use ``\begin{proof}[**thm:XYZ] ...`` to
  simply typeset `Proof` as usual, but in this way you have the information about
  which theorem the proof is directly at hand; if you later move it around there
  is no ambiguity as to which theorem the proof is associated with.  It is also
  the same syntax as supported by the [phfthm latex
  package](https://github.com/phfaist/phfqitltx) which is able to produce
  hyperlinks between theorems and their proofs.

## Document Front Matter

FLM Documents can contain YAML front matter that specify (i) options for the FLM
parser, (ii) which features to enable, and (iii) additional document metadata such
as a title.
```yaml
---
title: 'My FLM document'
flm:
   parsing:
     enable_dollar_math_mode: True
   features:
     endnotes:
       categories:
         - category_name: footnote
           counter_formatter: unicodesuperscript
           heading_title: 'Footnotes'
           endnote_command: 'footnote'
---

\section{Greeting}
Hello, \emph{world}. Inline math can now also be typeset
as $a$ and $b$.  ...

```

### Imports

You can use the `$import:` directive to import a configuration from an
external file, URL, or extension package:
```yaml
$import:
  - my-flm-config.yaml # merge my-flm-config.yaml into this config.

# you can still specify configuration to merge with here ...
...
flm:
   ...
```

The `$import:` target can specify multiple configurations to import.  Each list
item can be a absolute or relative file path (`$import: 'my-flm-config.yaml'` or
`$import: /path/to/my/flm-config.yaml`), a URL (`$import:
https://example.com/my/flm-config.yaml`), or a fully qualified python package
name introduced with ``pkg:package_name`` (e.g., `$import: pkg:flm_citations`).
If a package name is specified to the `$import` directive, the package is loaded
and the default FLM configuration is extracted from it and included (the
`flm_default_import_config` attribute of the module is read; it is assumed to be
a dictionary or a callable that returns a dictionary).  You can optionally
follow the package name by a path to specify submodules/attributes to read
instead of `flm_default_import_config`; e.g., ``pkg:mypackage/foo/bar`` will
import the module `mypackage` and import the configuration dictionary stored in
``mypackage.foo.bar``.  FLM extention plugin/package authors can use this
feature to offer preset customization configurations that can easily be included
with ``pkg:some_flm_extension_package/some/preset/name``.


### Parser configuration

Here's a basic parser configuration that you can adapt:
```yaml
flm:
  parsing:
    # Enable/Disable comments as in LaTeX, led by ‘%%’
    enable_comments: true
    # This string initiates a comment up to the end of
    # the line, if comments are enabled.  By default we
    # require TWO '%' signs to avoid accidental comments
    # (e.g., "... is up 10% from ...")
    comment_start: '%%'
    # Enable/Disable math mode with $ signs as in LaTeX
    # in addition to \( ...\)
    dollar_inline_math_mode: false
    # Force parsing of the content in block-level mode
    # (paragraphs).  Here ‘null’ means to auto-detect
    force_block_level: null
```

### Renderer configuration (`html`, `text`, `latex`, `markdown`)

Here's a basic renderer configuration that you can adapt **for HTML output**
(`--format=html`):
```yaml
flm:
  renderer:
    html:
      use_link_target_blank: false
      html_blocks_joiner: ''
      # use ‘headings_tags_by_level: null’ for defaults
      heading_tags_by_level:
        1: h1
        2: h2
        3: h3
        4: span
        5: span
        6: span
      inline_heading_add_space: true
```

Here's a basic renderer configuration that you can adapt **for text output**
(`--format=text`):
```yaml
flm:
  renderer:
    text:
      display_href_urls: true
```

Here's a basic renderer configuration that you can adapt **for LaTeX output**
(`--format=latex`):
```yaml
flm:
  renderer:
    latex:
      heading_commands_by_level:
        1: "section"
        2: "subsection"
        3: "subsubsection"
        4: "paragraph"
        5: "subparagraph"
        6: null
```

Here's a basic renderer configuration that you can adapt **for Markdown output**
(`--format=markdown`):
```yaml
flm:
  renderer:
    markdown:
      use_target_ids: 'github' # or 'anchor' or 'pandoc' or null
```


### Features and their configuration

Many FLM features are organized explicitly into feature classes which can be
enabled or disabled at wish.  Features include:

- enumeration (`\begin{enumerate}...\end{enumerate}`) and itemization
  (`\begin{itemize}...\end{itemize}`) lists
  
- floats: figures and tables (`\begin{figure}...\end{figure}`)

- headings (`\section{...}` etc.)

- etc.

Features can be selected and configured directly in the FLM config metadata.  For instance
the following configuration is extracted from the default feature configuration when you run
FLM:
```yaml
flm:
  features:
    # list features that should be available here.
    enumeration:
      enumeration_environments:
        enumerate:
          # here null means to use defaults
          counter_formatter: null
        itemize:
          counter_formatter:
            - "\u2022"
            - '-'
            - "\u25B8"
    refs: {}
    endnotes:
      categories:
        - category_name: footnote
          counter_formatter: alph
          endnote_command: footnote
          heading_title: Footnotes
      render_options:
        include_headings_at_level: 1
        set_headings_target_ids: true
        endnotes_heading_title: null
        endnotes_heading_level: 1
    floats:
      float_types:
        - counter_formatter: Roman
          float_caption_name: Fig.
          float_type: figure
          content_handlers: ['any', 'includegraphics', 'cells']
        - counter_formatter: Roman
          float_caption_name: Tab.
          float_type: table
          content_handlers: ['cells']
    defterm: {}
    graphics: {}
```

## Additional Features such as *Citations*

Additional features can be imported in the flm config.  They can reside in other
python packages.  Some day I'll properly document how to write new features.
For now, check out the examples in `flm/feature/xxx.py` (and keep in mind that
the APIs are still likely to change!).

To include for instance the citations feature provided by the
[flm-citations](https://github.com/phfaist/flm-citations) package, install that
package and use the config:
```yaml
$import:
  - pkg:flm_citations
bibliography:
  - bibpreset.yaml
  - anotherbibtest.json
```
Citations are organized by citation prefix and are automatically retrieved
depending on the type of citation.  By default:
- Citations of the form `arxiv:<arXiv ID>` are automatically retrieved from the arXiv API
- Citations of the form `doi:<DOI>` are automatically retrieved from [doi.org](https://doi.org/)
- Citations of the form `manual:{Manual citation}` add the manual citation itself as a citation
- Citations of the form `bib:key` look up `key` in one of the specified bibliography files.  The
  bibliography files are expected to be CSL-JSON or CSL-YAML files. (Sorry, no bibtex for now.)

You can of course configure all of that manually. Check out the code in the
[`flm-citations`](https://github.com/phfaist/flm-citations) repo for more insight.  Doc will
hopefully come soon.


# Using the `flm` package

Needs doc.

## Example. Converting FLM to HTML

Note: Math is simply marked with `<span class=...>` tags for use with
[MathJax](https://www.mathjax.org/).

Example:
```py
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer

environ = make_standard_environment(features=standard_features())

# suppose we have fragments of FLM text
fragment_1 = environ.make_fragment(r'Hello, \emph{world}.')
fragment_2 = environ.make_fragment(
    r'''Here's a question: \(1+2=?\)
\begin{enumerate}[(a)]
\item 1
\item 2
\item 3
\end{enumerate}
'''
)

# we can define a callback to render these fragments within an
# HTML page. The advantage of doing so is that different fragments
# can "see" each other! (E.g., for \ref's, etc.) This feature can
# be useful especially in combination with template engines.
def render_fn(render_context):
    return (
        "<main>\n"
        + "<div>" + fragment_1.render(render_context) + "</div>\n"
        + fragment_2.render(render_context) + "\n"
        + "</main>"
    )

doc = environ.make_document(render_fn)

fragment_renderer = HtmlFragmentRenderer()
result_html, render_context = doc.render(fragment_renderer)

print(result_html)
# *** Prints:
#
# <main>
# <div>Hello, <span class="textit">world</span>.</div>
# <p>Here&#x27;s a question: <span class="inline-math">\(1+2=?\)</span></p>
# <dl class="enumeration enumerate"><dt>(a)</dt><dd><p>1</p></dd><dt>(b)</dt><dd><p>2</p></dd><dt>(c)</dt><dd><p>3</p></dd></dl>
# </main>
```


# A Javascript FLM library

You can transpile the core part of this library to Javascript using Transcrypt.
See [the `flm-js` subfolder](flm-js/README.md) for more details.
