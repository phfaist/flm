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

This project is still early in its development stage, and a lot of features are
simply not yet implemented.

Planned features:

- everything described in [`llm_features.md`](llm_features.md)

- JavaScript version via [*Transcrypt*](https://transcrypt.org) !


# Converting LLM to HTML

Note: Math is simply marked with `<span class=...>` tags for use with
[MathJax](https://www.mathjax.org/).

Example:
```py
from llm import llmstd
from llm.htmlfragmentrenderer import HtmlFragmentRenderer

environ = llmstd.LLMStandardEnvironment()

# suppose we have fragments of LLM text
fragment_1 = environ.make_fragment(r'Hello, \emph{world}.')
fragment_2 = environ.make_fragment(
    r'Question: \(1+2=?\)'
)

# we can define a callback to render these fragments within an
# HTML page. The advantage of doing so is that different fragments
# can "see" each other! (E.g., for \ref's, etc.) This feature can
# be useful especially in combination with template engines.
def render_fn(doc, fragment_renderer):
    return (
        "<main>\n"
        + "<div>" + fragment_1.render(doc, fragment_renderer) + "</div>\n"
        + "<div>" + fragment_2.render(doc, fragment_renderer) + "</div>\n"
        + "</main>"
    )

doc = environ.make_document(render_fn)

fragment_renderer = HtmlFragmentRenderer()
result_html = doc.render(fragment_renderer)

print(result_html)
# *** Prints:
#
# <main>
# <div><p>Hello, <span class="textit">world</span>.</p></div>
# <div><p>Question: <span class="inline-math">\(1+2=?\)</span></p></div>
# </main>
```
