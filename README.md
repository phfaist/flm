# A simple Latex-Like Markup (LLM) language

This package provides a simple parser and formatter for a custom markup language
that is inspired by LaTeX syntax.

The syntax of LLM is essentially a subset of standard LaTeX commands, including
macros, environments, and some characters that have a special meaning; these
features are parsed in a loosely similar fashion to usual LaTeX code.

See [Latex-Like Markup Language Features](llm_features.md) for more some
standard features that you can expect with a default set of commands.

The framework is meant to be very easily extendible and customizable.  The
parser is based on [*pylatexenc 3*](https://github.com/phfaist/pylatexenc)
(which is currently still in development stage).

# This is work in progress!

This project is still early in an active development stage, and there might
still be a few bugs around. You can expect the API to still change pretty
drastically.

Planned features:

- Basically all the features that are available in the [Error Correction Zoo
  data files](https://github.com/errorcorrectionzoo/eczoo_generator/blob/main/latexlike_minilang_howto.md)
  
- Feel free to share ideas!

# A command-line tool

You can use `llm` in command-line mode to compile your documents:
```bash
$ pip install llm
$ llm mydocument.llm
```

Run `llm --help` to get a list of options. They should be self-explanatory:
```bash
$ # output to file mydocument.html, format HTML, including skeleton HTML with basic CSS
$ llm mydocument.llm -o mydocument.html --format=html --minimal-document
```

Available formats are `html`, `text`, and `latex`.  Formats `text` and `latex` are very
experimental!

## Document Front Matter

LLM Documents can contain YAML front matter that specify (i) options for the LLM
parser, (ii) which features to enable, and (iii) additional document metadata such
as a title.
```
---
title: 'My LLM document'
llm:
   features:
     - $preset: defaults
     - $preset: feature-config
       name: llm.feature.endnotes.FeatureEndnotes
       config:
         categories:
           - category_name: footnote
             counter_formatter: unicodesuperscript
             heading_title: 'Footnotes'
             endnote_command: 'footnote'
---

\section{Greeting}
Hello, \emph{world}. ...

```

As you can see, there are a few options you can set and a few special instructions
in the config that are led by the `$preset:` key.  The `$preset: defaults` item in
a list imports all the existing defaults for that list. The `$preset: feature-config`
is used to alter the configuration of an already-declared feature.  Additionally
you can use the `$preset: import` to import a configuration from an external file or
URL:
```
# merge my-llm-config.yaml into this config
$preset: import
$target: my-llm-config.yaml # also URLs like https://mysite.com/my-llm-config.yaml
llm:
   ... # can still specify configuration to merge with here ...
```

## Additional Features such as *Citations*

Additional features can be imported in the llm config.  They can reside in other python packages.
Some day I'll properly document how to write new features.  For now, check out the examples
in `llm/feature/xxx.py` (and keep in mind that the APIs are still likely to change!).

To include for instance the citations feature provided by the
[llm-citations](https://github.com/phfaist/llm-citations) package, install that package and
use the config:
```
llm:
  features:
    - $preset: defaults
    - name: 'llm_citations.feature_cite.FeatureCiteAuto'
bibliography:
  - bibpreset.yaml
  - anotherbibtest.json
```
Citations are organized by citation prefix and are automatically retrieved depending on the
type of citation.  By default:
- Citations of the form `arxiv:<arXiv ID>` are automatically retrieved from the arXiv API
- Citations of the form `doi:<DOI>` are automatically retrieved from [doi.org](https://doi.org/)
- Citations of the form `manual:{Manual citation}` add the manual citation itself as a citation
- Citations of the form `bib:key` look up `key` in one of the specified bibliography files.  The
  bibliography files are expected to be CSL-JSON or CSL-YAML files. (Sorry, no bibtex for now.)

You can of course configure all of that manually. Check out the code in the
[`llm-citations`](https://github.com/phfaist/llm-citations) repo for more insight.  Doc will
hopefully come soon.


# Using the `llm` package

Needs doc.

## Example. Converting LLM to HTML

Note: Math is simply marked with `<span class=...>` tags for use with
[MathJax](https://www.mathjax.org/).

Example:
```py
from llm import llmstd
from llm.fragmentrenderer.html import HtmlFragmentRenderer

environ = llmstd.LLMStandardEnvironment()

# suppose we have fragments of LLM text
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


# A Javascript LLM library

You can transpile this library to Javascript using Transcrypt.  See [the
`llm-js` subfolder](llm-js/README.md) for more details.
