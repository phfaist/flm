import unittest

from llm.llmdocument import LLMDocument
from llm.fragmentrenderer import TextFragmentRenderer
from llm.htmlfragmentrenderer import HtmlFragmentRenderer
from llm.llmstd import LLMStandardEnvironment


class TestLLMDocument(unittest.TestCase):

    maxDiff = None

    def test_simple_html(self):
    
        environ = LLMStandardEnvironment()

        frag1 = environ.make_llm_fragment(r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).")
        frag2 = environ.make_llm_fragment(
            r"""
We can also split text across multiple paragraphs, like this
block of text here.

we can also have an equation, like this:
\begin{align}
    1 + 3 - 5 = -1
\end{align}
            """.strip()
        )

        def render_fn(docobj, frobj):
            return (
                "<main>\n"
                "<div>" + frag1.render(docobj, frobj) + "</div>\n"
                "<div>" + frag2.render(docobj, frobj) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()
        doc = LLMDocument(render_fn, environ, fr)

        result = doc.render()
        print(result)

        self.assertEqual(result, r"""
<main>
<div><p><span class="textbf">&lt;p&gt;Hello&lt;/p&gt;</span> <span class="textit">&lt;p&gt;world&lt;/p&gt;</span>, we know that <span class="inline-math">\(a+b=c\)</span>.</p></div>
<div><p>We can also split text across multiple paragraphs, like this
block of text here.</p>
<p>we can also have an equation, like this:
<span class="display-math env-align">\begin{align}
    1 + 3 - 5 = -1
\end{align}</span></p></div>
</main>
        """.strip())
        

    def test_simple_text(self):
    
        environ = LLMStandardEnvironment()

        frag1 = environ.make_llm_fragment(r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).")
        frag2 = environ.make_llm_fragment(
            r"""
We can also split text across multiple paragraphs, like this
block of text here.

we can also have an equation, like this:
\begin{align}
    1 + 3 - 5 = -1
\end{align}
            """.strip()
        )

        def render_fn(docobj, frobj):
            return (
                "|||\n" + frag1.render(docobj, frobj) + "\n|||\n"
                + frag2.render(docobj, frobj) + "\n|||"
            )

        fr = TextFragmentRenderer()
        doc = LLMDocument(render_fn, environ, fr)

        result = doc.render()
        print(result)

        self.assertEqual(result, r"""
|||
Hello world, we know that \(a+b=c\).
|||
We can also split text across multiple paragraphs, like this
block of text here.

we can also have an equation, like this:
\begin{align}
    1 + 3 - 5 = -1
\end{align}
|||
        """.strip())
        
