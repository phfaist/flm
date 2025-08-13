import unittest
import re

from pylatexenc.latexnodes import LatexWalkerParseError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer

from flm.feature import refs as feature_refs



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class TestStandardFeatures(unittest.TestCase):

    maxDiff = None

    def test_no_comments(self):

        environ = mk_flm_environ()

        self.assertTrue(environ.parsing_state.enable_comments)
        self.assertEqual(environ.parsing_state.comment_start, '%%')
        self.assertEqual(environ.parsing_state.forbidden_characters, '$%')

        with self.assertRaises(LatexWalkerParseError):
            frag1 = environ.make_fragment(
                r"\textbf{Hello} \textit{world}. % Comments are forbidden."
            )



    def test_make_document(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(a+b=c\)."
        )

        def render_fn(render_context):
            return (
                "<main>\n"
                "<div>" + frag1.render(render_context, is_block_level=True) + "</div>\n"
                "</main>"
            )

        fr = HtmlFragmentRenderer()

        # FLMEnvironment offers make_document to avoid having to import and
        # instantiate FLMDocuments ourselves.
        doc = environ.make_document(render_fn)

        result, _ = doc.render(fr)

        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.</p></div>
</main>""".strip()
        )


    def test_with_math_and_eqref(self):

        environ = mk_flm_environ()

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

        def render_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p><span class="textbf">Hello</span>, see <a href="#equation-1" class="href-ref ref-eq">(1)</a>.</p><p>Here is the equation: <span id="equation-1" class="display-math env-align">\begin{align}
  \label{eq:my-equation}
  \int f(x)\, dx = -1\ .
\tag*{(1)}\end{align}</span></p>
""".strip()
        )



    def test_multi_equation_and_eqref(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"""
\textbf{Hello}, see \eqref{eq:my-equation}, \eqref{eq:2}, and \eqref{eq:3}.

Here is the equation:
\begin{align}
  \label{eq:my-equation}
  \int f(x)\, dx = -1\ .
            \\
\label{eq:2}
  A + B = C
    \tag{A}
  \\
  D + E = F
  \tag*{-B-}
\label{eq:3}
\\
  \cdots
\end{align}
""".strip()
        )

        def render_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        result_fix = re.sub(r'(#equation-)\d{2,}', r'\1???', result)
        self.assertEqual(
            result_fix,
            r"""
<p><span class="textbf">Hello</span>, see <a href="#equation-1" class="href-ref ref-eq">(1)</a>, <a href="#equation-1" class="href-ref ref-eq">(A)</a>, and <a href="#equation-1" class="href-ref ref-eq">-B-</a>.</p><p>Here is the equation: <span id="equation-1" class="display-math env-align">\begin{align}
  \label{eq:my-equation}
  \int f(x)\, dx = -1\ .
            \tag*{(1)}\\
\label{eq:2}
  A + B = C
    \tag{A}
  \\
  D + E = F
  \tag*{-B-}
\label{eq:3}
\\
  \cdots
\tag*{(2)}\end{align}</span></p>
""".strip()
        )



    def test_unknown_macros_in_math(self):
        environ = mk_flm_environ()
        frag1 = environ.make_fragment(
            r"We know that \(\alpha+\beta=\gamma\)."
        )
        self.assertEqual(frag1.nodes[1].nodelist[0].macroname, 'alpha')

    def test_no_unknown_macros_in_text(self):
        environ = mk_flm_environ()
        with self.assertRaises(LatexWalkerParseError):
            frag1 = environ.make_fragment(
                r"We know that \unknownMacros can cause errors."
            )




    def test_standalone_fragment_with_math(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"""Variable \(x\)""",
            standalone_mode=True
        )

        def render_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""Variable <span class="inline-math">\(x\)</span>"""
        )






    def test_provides_href(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \href{https://github.com/}{\textit{world}}, check out our "
            r"\url{https://errorcorrectionzoo.org/?}."
        )

        def render_fn(render_context):
            return frag1.render(render_context, is_block_level=True)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p><span class="textbf">Hello</span> <a href="https://github.com/" class="href-href"><span class="textit">world</span></a>, check out our <a href="https://errorcorrectionzoo.org/?" class="href-href">errorcorrectionzoo.org</a>.</p>""".strip()
        )



    def test_provides_itemize(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
r"""
\begin{itemize}
\item First
\item Second
\end{itemize}
"""
        )

        def render_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<dl class="enumeration itemize"><dt>•</dt><dd><p>First</p></dd><dt>•</dt><dd><p>Second</p></dd></dl>
""".strip()
        )

    def test_provides_itemize_custombullets(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
r"""
\begin{itemize}[*]
\item First
\item[-] Second
\end{itemize}
"""
        )

        def render_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<dl class="enumeration itemize"><dt>*</dt><dd><p>First</p></dd><dt>-</dt><dd><p>Second</p></dd></dl>
""".strip()
        )

    def test_provides_enumerate_custombullets(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
r"""
\begin{enumerate}[{(a.)}]
\item First
\item[!] Second
\item Third
\end{enumerate}
"""
        )

        def render_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = ''

        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<dl class="enumeration enumerate"><dt>(a.)</dt><dd><p>First</p></dd><dt>!</dt><dd><p>Second</p></dd><dt>(c.)</dt><dd><p>Third</p></dd></dl>
""".strip()
        )


    def test_provides_footnotes_by_default(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).\footnote{It is "
            r"\textit{true}!} I checked it."
        )

        def render_fn(render_context):
            return (
                "<main>\n"
                "<div>" + frag1.render(render_context, is_block_level=True) + "</div>\n"
                "</main>"
            )

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = "\n"

        result, render_context = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a> I checked it.</p></div>
</main>""".strip()
        )

        endnotes_result = str(
            render_context.feature_render_manager('endnotes').render_endnotes(
                target_id='my-endnotes'
            )
        )
        print(endnotes_result)
        self.assertEqual(
            endnotes_result,
r'''<div id="my-endnotes" class="endnotes"><dl class="enumeration footnote-list"><dt id="footnote-1">a</dt><dd>It is <span class="textit">true</span>!</dd></dl></div>'''
)


    def test_provides_citations_by_default_if_given_external_citations_provider(self):

        environ = None

        class MyCitationsProvider:
            def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
                if cite_prefix == 'arxiv':
                    # can return FLM text as a string
                    return r'\textit{arXiv} paper ' + f'arXiv:{cite_key}'
                if cite_prefix == 'manual':
                    # can return a compiled fragment
                    return environ.make_fragment(
                        cite_key,
                        is_block_level=False,
                        standalone_mode=True,
                        what=f"Custom manual citation text",
                    )
                raise ValueError(f"Invalid citation prefix: {cite_prefix!r}")

        environ = mk_flm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(
            r"\textbf{Hello} \textit{world}, we know that \(a+b=c\).\footnote{It is "
            r"\textit{true}!} I checked it~\cite{arxiv:1234.56789}.  And "
            r"again and again~\cite{arxiv:0000.11111,manual:{My custom "
            r"\emph{reference} (2022).},arxiv:3333.99999}.  See "
            r"also \cite[Theorem~3]{arXiv:0000.11111}."
        )

        def render_fn(render_context):
            return (
                "<main>\n"
                "<div>" + frag1.render(render_context, is_block_level=True) + "</div>\n"
                "</main>"
            )

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        result, render_context = doc.render(fr, {
            'citations': {
                'sort_and_compress': False
            }
        })
        print(result)
        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a> I checked it&nbsp;<span class="citation-marks"><a href="#citation-1" class="href-endnote endnote citation">[1]</a></span>. And again and again&nbsp;<span class="citation-marks"><a href="#citation-2" class="href-endnote endnote citation">[2]</a><a href="#citation-3" class="href-endnote endnote citation">[3]</a><a href="#citation-4" class="href-endnote endnote citation">[4]</a></span>. See also <span class="citation-marks"><a href="#citation-2" class="href-endnote endnote citation">[2; Theorem&nbsp;3]</a></span>.</p></div>
</main>
""".strip()
        )

        endnotes_result = str(
            render_context.feature_render_manager('endnotes').render_endnotes(target_id=None)
        )
        print(endnotes_result)
        self.assertEqual(
            endnotes_result,
            r"""
<div class="endnotes"><dl class="enumeration footnote-list"><dt id="footnote-1">a</dt><dd>It is <span class="textit">true</span>!</dd></dl>
<dl class="enumeration citation-list"><dt id="citation-1">[1]</dt><dd><span class="textit">arXiv</span> paper arXiv:1234.56789</dd><dt id="citation-2">[2]</dt><dd><span class="textit">arXiv</span> paper arXiv:0000.11111</dd><dt id="citation-3">[3]</dt><dd>My custom <span class="textit">reference</span> (2022).</dd><dt id="citation-4">[4]</dt><dd><span class="textit">arXiv</span> paper arXiv:3333.99999</dd></dl></div>
            """.strip()
)



    def test_renders_latex_with_citations(self):

        environ = None

        class MyCitationsProvider:
            def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
                if cite_prefix == 'arxiv':
                    # can return FLM text as a string
                    return r'\textbf{arXiv:' + cite_key + '}'
                raise ValueError(f"Invalid citation prefix: {cite_prefix!r}")

        environ = mk_flm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(
            r"""
Hello world~\cite{arxiv:1234.56789}.
""" .strip()
        )

        def render_fn(render_context):
            r = frag1.render(render_context)
            r += '\n'
            r += render_context.feature_render_manager('endnotes').render_endnotes()
            return r

        doc = environ.make_document(render_fn)

        fr = LatexFragmentRenderer()
        fr.debug_disable_pin_labels = True
        fr.debug_disable_link_hyperref = True

        result, render_context = doc.render(fr)
        print(result)
        self.assertEqual(
            result .replace('%\n', '')
              .replace('% --- begin  ---\n', '')
              .replace('% --- end  ---\n', '')
              .strip(),
            r"""
Hello world~[1].
\begin{itemize}% enumeration,citation-list
\item[{[1]}]\textbf{arXiv:1234.56789}\end{itemize}
""".strip()
        )


    def test_renders_latex_with_enumerate(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"""
\begin{enumerate}
\item One
\item Two
\item[c] Three
\end{enumerate}
""" .strip()
        )

        def render_fn(render_context):
            r = frag1.render(render_context)
            return r

        doc = environ.make_document(render_fn)

        fr = LatexFragmentRenderer()
        fr.debug_disable_pin_labels = True
        fr.debug_disable_link_hyperref = True

        result, render_context = doc.render(fr)
        print(result)
        self.assertEqual(
            result .replace('%\n', '')
              .replace('% --- begin  ---\n', '')
              .replace('% --- end  ---\n', '')
              .strip(),
            r"""
\begin{enumerate}% enumeration,enumerate
\item[{1.}]One
\item[{2.}]Two
\item[{c}]Three\end{enumerate}
""".strip()
        )


    def test_renders_html_with_citations(self):

        environ = None

        class MyCitationsProvider:
            def get_citation_full_text_flm(self, cite_prefix, cite_key, resource_info):
                if cite_prefix == 'arxiv':
                    # can return FLM text as a string
                    return r'arXiv:' + cite_key
                raise ValueError(f"Invalid citation prefix: {cite_prefix!r}")

        environ = mk_flm_environ(
            external_citations_providers=[ MyCitationsProvider() ]
        )

        frag1 = environ.make_fragment(
            r"""
Hello world~\cite{arxiv:1234.56789}.
""" .strip(),
            is_block_level=True,
        )

        def render_fn(render_context):
            r = frag1.render(render_context)
            r += '\n'
            r += render_context.feature_render_manager('endnotes').render_endnotes()
            return r

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()

        result, render_context = doc.render(fr)
        print(result)
        self.assertEqual(
            result.strip(),
            r"""
<p>Hello world&nbsp;<span class="citation-marks"><a href="#citation-1" class="href-endnote endnote citation">[1]</a></span>.</p>
<div id="endnotes" class="endnotes"><dl class="enumeration citation-list"><dt id="citation-1">[1]</dt><dd>arXiv:1234.56789</dd></dl></div>
""".strip()
        )




    def test_simple_ref_external(self):

        class MyRefResolver:
            def get_ref(self, ref_type, ref_label, resource_info, render_context):
                if ref_type == 'code':
                    if ref_label == 'surface':
                        return feature_refs.RefInstance(
                            ref_type='code',
                            ref_label='surface',
                            formatted_ref_flm_text=r'Kitaev \emph{surface} code',
                            target_href='https://errorcorrectionzoo.org/c/surface',
                            counter_value=None,
                            counter_numprefix=None,
                            counter_formatter_id=None,
                        )
                    if ref_label == 'self':
                        return feature_refs.RefInstance(
                            ref_type='code',
                            ref_label='self',
                            formatted_ref_flm_text=r'(\textbf{this} exact code)',
                            target_href='#',
                            counter_value=None,
                            counter_numprefix=None,
                            counter_formatter_id=None,
                        )
                    raise ValueError(f"Invalid ref target: ‘{ref_type}:{ref_label}’")
                raise ValueError(f"Invalid ref type: {ref_type!r}")


        environ = mk_flm_environ(
            external_ref_resolvers=[MyRefResolver()]
        )

        frag1 = environ.make_fragment(
            r"""
\textbf{Hello}, reference to \ref{code:self}. Check out the \ref{code:surface}, too.
""".strip()
        )

        def render_fn(render_context):
            return frag1.render(render_context, is_block_level=True)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p><span class="textbf">Hello</span>, reference to <a href="#" class="href-ref ref-code">(<span class="textbf">this</span> exact code)</a>. Check out the <a href="https://errorcorrectionzoo.org/c/surface" class="href-ref ref-code">Kitaev <span class="textit">surface</span> code</a>, too.</p>
""".strip()
        )



    def test_inner_math_mode_changes(self):
        
        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"""
Here is a display equation:
\begin{align}
  S_1 &= I\,Z\,X\,X\,Z\ ;  \nonumber\\
  S_2, \ldots, S_4 &= \text{cyclical permutations of \(S_1\)}\ .
\end{align}
""".lstrip(),
            is_block_level=True
        )
        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p>Here is a display equation: <span id="equation-1" class="display-math env-align">\begin{align}
  S_1 &amp;= I\,Z\,X\,X\,Z\ ;  \nonumber\\
  S_2, \ldots, S_4 &amp;= \text{cyclical permutations of \(S_1\)}\ .
\tag*{(1)}\end{align}</span></p>
""".strip()
        )





if __name__ == '__main__':
    unittest.main()
