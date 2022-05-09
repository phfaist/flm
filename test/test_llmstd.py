import unittest

from pylatexenc.latexnodes import LatexWalkerParseError

from llm.llmstd import LLMStandardEnvironment
from llm.htmlfragmentrenderer import HtmlFragmentRenderer

from llm.feature_endnotes import FeatureEndnotes, EndnoteCategory


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



    def test_with_math_and_eqref(self):

        environ = LLMStandardEnvironment()

        frag1 = environ.make_fragment(
            r"""
\textbf{Hello}, see \eqref{eq:my-equation}.

Here is the equation:
\begin{align}
  \label{eq:my-equation}
  \int f(x)\, dx = -1\ .
\end{align}
""".strip()
        )

        def render_fn(docobj, frobj):
            return frag1.render(docobj, frobj)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        result = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p><span class="textbf">Hello</span>, see <span class="inline-math">\(\eqref{eq:my-equation}\)</span>.</p>
<p>Here is the equation:
<span class="display-math env-align">\begin{align}
  \label{eq:my-equation}
  \int f(x)\, dx = -1\ .
\end{align}</span></p>
""".strip()
        )



    def test_unknown_macros_in_math(self):
        environ = LLMStandardEnvironment()
        frag1 = environ.make_fragment(
            r"We know that \(\alpha+\beta=\gamma\)."
        )
        self.assertEqual(frag1.nodes[1].nodelist[0].macroname, 'alpha')

    def test_no_unknown_macros_in_text(self):
        environ = LLMStandardEnvironment()
        with self.assertRaises(LatexWalkerParseError):
            frag1 = environ.make_fragment(
                r"We know that \unknownMacros can cause errors."
            )



    def test_provides_href(self):

        environ = LLMStandardEnvironment()

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \href{https://github.com/}{\textit{world}}, check out our "
            r"\url{https://errorcorrectionzoo.org/?}."
        )

        def render_fn(docobj, frobj):
            return frag1.render(docobj, frobj)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        result = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p><span class="textbf">Hello</span> <a href="https://github.com/" class="href-href"><span class="textit">world</span></a>, check out our <a href="https://errorcorrectionzoo.org/?" class="href-href">errorcorrectionzoo.org</a>.</p>""".strip()
        )


    def test_provides_footnotes_by_default(self):

        environ = LLMStandardEnvironment()

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).\footnote{It is "
            r"\textit{true}!} I checked it."
        )

        def render_fn(docobj, frobj):
            return (
                "<main>\n"
                "<div>" + frag1.render(docobj, frobj) + "</div>\n"
                "</main>"
            )

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        result = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.<a href="#footnote-1" class="href-endnote endnote footnote">a</a> I checked it.</p></div>
</main>""".strip()
        )

        endnotes_result = doc.feature_manager('endnotes').render_endnotes(fr)
        print(endnotes_result)
        self.assertEqual(
            endnotes_result,
r'''<div class="endnotes"><dl class="enumeration footnote-list"><dt>a</dt><dd>It is <span class="textit">true</span>!</dd></dl><dl class="enumeration citation-list"></dl></div>'''
)


    def test_provides_citations_by_default_if_given_external_citations_provider(self):

        class MyCitationsProvider:
            def get_citation_full_text_llm(self, cite_prefix, cite_key):
                if cite_prefix == 'arxiv':
                    return r'\textit{arXiv} paper ' + f'arXiv:{cite_key}'
                if cite_prefix == 'manual':
                    return cite_key
                raise ValueError(f"Invalid citation prefix: {cite_prefix!r}")

        environ = LLMStandardEnvironment(
            external_citations_provider=MyCitationsProvider()
        )

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).\footnote{It is "
            r"\textit{true}!} I checked it~\cite{arxiv:1234.56789}.  And "
            r"again and again~\cite{arxiv:0000.11111,manual:{My custom "
            r"\emph{reference} (2022).},arxiv:3333.99999}"
        )

        def render_fn(docobj, frobj):
            return (
                "<main>\n"
                "<div>" + frag1.render(docobj, frobj) + "</div>\n"
                "</main>"
            )

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        result = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.<a href="#footnote-1" class="href-endnote endnote footnote">a</a> I checked it <a href="#citation-1" class="href-endnote endnote citation">[1]</a>.  And again and again <a href="#citation-2" class="href-endnote endnote citation">[2]</a><a href="#citation-3" class="href-endnote endnote citation">[3]</a><a href="#citation-4" class="href-endnote endnote citation">[4]</a></p></div>
</main>
""".strip()
        )

        endnotes_result = doc.feature_manager('endnotes').render_endnotes(fr)
        print(endnotes_result)
        self.assertEqual(
            endnotes_result,
            r"""
<div class="endnotes"><dl class="enumeration footnote-list"><dt>a</dt><dd>It is <span class="textit">true</span>!</dd></dl><dl class="enumeration citation-list"><dt>[1]</dt><dd><p><span class="textit">arXiv</span> paper arXiv:1234.56789</p></dd><dt>[2]</dt><dd><p><span class="textit">arXiv</span> paper arXiv:0000.11111</p></dd><dt>[3]</dt><dd><p><p>My custom <span class="textit">reference</span> (2022).</p></p></dd><dt>[4]</dt><dd><p><span class="textit">arXiv</span> paper arXiv:3333.99999</p></dd></dl></div>
            """.strip()
)



if __name__ == '__main__':
    unittest.main()
