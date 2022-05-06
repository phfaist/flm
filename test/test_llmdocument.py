import unittest

from llm.llmdocument import LLMDocument
from llm.htmlfragmentrenderer import HtmlFragmentRenderer
from llm.llmstd import LLMStandardEnvironment


class TestLLMDocument(unittest.TestCase):

    def test_simple(self):
    
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

        def render_fn(docobj, fr):
            return (
                "<main>\n"
                "<div>" + frag1.render(docobj, fr) + "</div>\n"
                "<div>" + frag2.render(docobj, fr) + "</div>\n"
            )

        htmlfr = HtmlFragmentRenderer()
        doc = LLMDocument(render_fn, environ, htmlfr,)

        result = doc.render()

        print(result)

        self.assertTrue(False)
        
