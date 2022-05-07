import unittest

from pylatexenc.latexnodes import LatexWalkerParseError

from llm.llmstd import LLMStandardEnvironment
from llm.htmlfragmentrenderer import HtmlFragmentRenderer


class TestLLMStandardEnvironment(unittest.TestCase):

    def test_no_comments(self):

        environ = LLMStandardEnvironment()

        with self.assertRaises(LatexWalkerParseError):
            frag1 = environ.make_fragment(
                r"\textbf{Hello} \textit{world}. % Comments are forbidden."
            )

        #fr = HtmlFragmentRenderer()
        #result = fr.render_fragment(frag1, doc=None)



    def test_make_document(self):

        environ = LLMStandardEnvironment()

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(a+b=c\)."
        )

        def render_fn(docobj, frobj):
            return (
                "<main>\n"
                "<div>" + frag1.render(docobj, frobj) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()

        # LLMStandardEnvironment offers make_document to avoid having to import
        # and instantiate LLMDocuments ourselves.
        doc = environ.make_document(render_fn)

        result = doc.render(fr)

        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.</p></div>
</main>""".strip()
        )




if __name__ == '__main__':
    unittest.main()
