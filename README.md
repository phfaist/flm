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

This project is still early in its development stage, and there might still be
a few bugs around.

Planned features:

- everything described in [`llm_features.md`](llm_features.md)

# A command-line tool

You can use `llm` in command-line mode to compile your documents:
```
> python -m llm mydocument.llm
```


# A Javascript LLM library

You can transpile this library to Javascript using Transcrypt.  See [the
`llm-js` subfolder](llm-js/README.md) for more details.

# Example. Converting LLM to HTML

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
