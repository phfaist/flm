import unittest
import re

from pylatexenc.latexnodes import LatexWalkerParseError

from llm.llmenvironment import make_standard_environment
from llm.stdfeatures import standard_features
from llm.fragmentrenderer.html import HtmlFragmentRenderer

from llm.feature.endnotes import FeatureEndnotes, EndnoteCategory
from llm.feature import refs as feature_refs



def mk_llm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class TestStandardFeatures(unittest.TestCase):

    maxDiff = None

    def test_no_comments(self):

        environ = mk_llm_environ()

        self.assertTrue(environ.parsing_state.enable_comments)
        self.assertEqual(environ.parsing_state.comment_start, '%%')
        self.assertEqual(environ.parsing_state.forbidden_characters, '$%')

        with self.assertRaises(LatexWalkerParseError):
            frag1 = environ.make_fragment(
                r"\textbf{Hello} \textit{world}. % Comments are forbidden."
            )



    def test_make_document(self):

        environ = mk_llm_environ()

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

        # LLMEnvironment offers make_document to avoid having to import and
        # instantiate LLMDocuments ourselves.
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

        environ = mk_llm_environ()

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

        environ = mk_llm_environ()

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
        environ = mk_llm_environ()
        frag1 = environ.make_fragment(
            r"We know that \(\alpha+\beta=\gamma\)."
        )
        self.assertEqual(frag1.nodes[1].nodelist[0].macroname, 'alpha')

    def test_no_unknown_macros_in_text(self):
        environ = mk_llm_environ()
        with self.assertRaises(LatexWalkerParseError):
            frag1 = environ.make_fragment(
                r"We know that \unknownMacros can cause errors."
            )




    def test_standalone_fragment_with_math(self):

        environ = mk_llm_environ()

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

        environ = mk_llm_environ()

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

        environ = mk_llm_environ()

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

        environ = mk_llm_environ()

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

        environ = mk_llm_environ()

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

        environ = mk_llm_environ()

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
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.<a href="#footnote-1" class="href-endnote endnote footnote">a</a> I checked it.</p></div>
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
            def get_citation_full_text_llm(self, cite_prefix, cite_key, resource_info):
                if cite_prefix == 'arxiv':
                    # can return LLM text as a string
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

        environ = mk_llm_environ(
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
        result, render_context = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<main>
<div><p><span class="textbf">Hello</span> <span class="textit">world</span>, we know that <span class="inline-math">\(a+b=c\)</span>.<a href="#footnote-1" class="href-endnote endnote footnote">a</a> I checked it <a href="#citation-1" class="href-endnote endnote citation">[1]</a>. And again and again <a href="#citation-2" class="href-endnote endnote citation">[2]</a><a href="#citation-3" class="href-endnote endnote citation">[3]</a><a href="#citation-4" class="href-endnote endnote citation">[4]</a>. See also <a href="#citation-2" class="href-endnote endnote citation">[2; Theorem 3]</a>.</p></div>
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




    def test_simple_ref_external(self):

        class MyRefResolver:
            def get_ref(self, ref_type, ref_label, resource_info, render_context):
                if ref_type == 'code':
                    if ref_label == 'surface':
                        return feature_refs.RefInstance(
                            ref_type='code',
                            ref_label='surface',
                            formatted_ref_llm_text=r'Kitaev \emph{surface} code',
                            target_href='https://errorcorrectionzoo.org/c/surface',
                        )
                    if ref_label == 'self':
                        return feature_refs.RefInstance(
                            ref_type='code',
                            ref_label='self',
                            formatted_ref_llm_text=r'(\textbf{this} exact code)',
                            target_href='#',
                        )
                    raise ValueError(f"Invalid ref target: ‘{ref_type}:{ref_label}’")
                raise ValueError(f"Invalid ref type: {ref_type!r}")


        environ = mk_llm_environ(
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
        
        environ = mk_llm_environ()

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
  S_1 &amp;= I\,Z\,X\,X\,Z\ ;  \nonumber\tag*{(1)}\\
  S_2, \ldots, S_4 &amp;= \text{cyclical permutations of \(S_1\)}\ .
\tag*{(2)}\end{align}</span></p>
""".strip()
        )








    def test_larger_doc_does_this_work(self):

        environ = mk_llm_environ()

        frag1 = environ.make_fragment(
            r"""Hello, world. \emph{Here I am.}  What would you like
to    see?

    \begin{itemize}
  \item cool things?
  \item more stuff?
  \item[!!!] Fun stuff??
\end{itemize}

\section{First section}

From there to here, funny things are everywhere.

\begin{defterm}{Pauli matrices}
  The \emph{Pauli matrices} are defined as
  \begin{align}
    \sigma_X = \begin{pmatrix} 0 & 1\\1 & 0\end{pmatrix}\ ;
    \sigma_Y = \begin{pmatrix} 0 & -i\\i & 0\end{pmatrix}\ ;
    \sigma_Z = \begin{pmatrix} 1 & 0\\0 & -1\end{pmatrix}\ .
  \end{align}
\end{defterm}

\begin{defterm}{qu\(d\)it}
  \label{topic:qudit}
  A \emph{qu\(d\)it} is a \(d\)-dimensional quantum system.
\end{defterm}

Here's a display equation:
\begin{align}
  a + b = c\ .
  \label{eq:my-equation}
\end{align}

\subsection{An enumeration list}

\begin{enumerate}[{(a.)}]
\item First thing to see could be something nice.  This line might also be
  pretty long, with lots of unnecessary text that I could have imagined cutting
  off at some point.

\item Second nice thing!  Check also \eqref{eq:my-equation} and \ref{figure:my-figure}.
\end{enumerate}

\paragraph{A paragraph-level sectioning command}
Paragraph content goes here. How does this work?  I can add a
figure, too; let's see how it works.
\begin{figure}
  \includegraphics{https://media.giphy.com/media/8Iv5lqKwKsZ2g/giphy.gif}
  \caption{This figure has a caption.}
  \label{figure:my-figure}
\end{figure}

Don't forget how the \term{Pauli matrices} are defined.  Also, \(\sigma_X\) is a
\hyperref[defterm:{Pauli matrices}]{Pauli matrix} (or a \term[Pauli
matrices]{Pauli matrix}).  Recall the definition of a \term{qu\(d\)it}, noting
that a qubit is also a \term[qu\(d\)it]{qu\(d=2\)it}.

\begin{verbatimtext}   Now \!this%$%^}] <is> some text that
   will be rendered
verbatim.
as \{is.\end{verbatim\ ha no, gotcha parser!
  \end{verbatimtext}

\emph{Vertaim macros like \verb+}+ and \verb|{| should be
  parsed correctly, even as macro arguments!}

\section{Another section}
\label{sec:test-label-another-sec}

We can try to add some footnotes, too.\footnote{Like this! Does it work?}  Does it work?

See also the topic \ref{topic:qudit}.

"""
        )

        def render_fn(render_context):
            return frag1.render(render_context, is_block_level=True)

        doc = environ.make_document(render_fn)

        fr = HtmlFragmentRenderer()
        fr.html_blocks_joiner = "\n"
        result, render_context = doc.render(fr)

        # add footnotes
        endnotes_mgr = render_context.feature_render_manager('endnotes')
        result = render_context.fragment_renderer.render_join_blocks([
            result,
            render_context.fragment_renderer.render_heading(
                environ.make_fragment('Footnotes').nodes,
                heading_level=1,
                render_context=render_context,
            ),
            endnotes_mgr.render_endnotes_category('footnote'),
        ])

        print(result)
        self.assertEqual(
            result,
            r"""
<p>Hello, world. <span class="textit">Here I am.</span> What would you like to see?</p>
<dl class="enumeration itemize"><dt>•</dt><dd><p>cool things?</p></dd><dt>•</dt><dd><p>more stuff?</p></dd><dt>!!!</dt><dd><p>Fun stuff??</p></dd></dl>
<h1 id="sec--First-section" class="heading-level-1">First section</h1>
<p>From there to here, funny things are everywhere.</p>
<div id="defterm-Pauli_20Xmatrices" class="defterm"><p><span class="defterm-term">Pauli matrices: </span>The <span class="textit">Pauli matrices</span> are defined as <span id="equation-1" class="display-math env-align">\begin{align}
    \sigma_X = \begin{pmatrix} 0 &amp; 1\\1 &amp; 0\end{pmatrix}\ ;
    \sigma_Y = \begin{pmatrix} 0 &amp; -i\\i &amp; 0\end{pmatrix}\ ;
    \sigma_Z = \begin{pmatrix} 1 &amp; 0\\0 &amp; -1\end{pmatrix}\ .
  \tag*{(1)}\end{align}</span></p></div>
<div id="defterm-qu_5cX_28Xd_5cX_29Xit" class="defterm"><p><span class="defterm-term">qu<span class="inline-math">\(d\)</span>it: </span>A <span class="textit">qu<span class="inline-math">\(d\)</span>it</span> is a <span class="inline-math">\(d\)</span>-dimensional quantum system.</p></div>
<p>Here&#x27;s a display equation: <span id="equation-2" class="display-math env-align">\begin{align}
  a + b = c\ .
  \label{eq:my-equation}
\tag*{(2)}\end{align}</span></p>
<h2 id="sec--An-enumeration-list" class="heading-level-2">An enumeration list</h2>
<dl class="enumeration enumerate"><dt>(a.)</dt><dd><p>First thing to see could be something nice. This line might also be pretty long, with lots of unnecessary text that I could have imagined cutting off at some point.</p></dd><dt>(b.)</dt><dd><p>Second nice thing! Check also <a href="#equation-2" class="href-ref ref-eq">(2)</a> and <a href="#figure-1" class="href-ref ref-figure">Figure 1</a>.</p></dd></dl>
<p><span id="sec--A-paragraph-level-sectionin" class="heading-level-4 heading-inline">A paragraph-level sectioning command</span> Paragraph content goes here. How does this work? I can add a figure, too; let&#x27;s see how it works.</p>
<figure id="figure-1" class="float float-figure"><div class="float-contents"><img src="https://media.giphy.com/media/8Iv5lqKwKsZ2g/giphy.gif"></div>
<figcaption class="float-caption-content"><span><span class="float-number">Figure&nbsp;1</span>: This figure has a caption.</span></figcaption></figure>
<p>Don&#x27;t forget how the <a href="#defterm-Pauli_20Xmatrices" class="href-term">Pauli matrices</a> are defined. Also, <span class="inline-math">\(\sigma_X\)</span> is a <a href="#defterm-Pauli_20Xmatrices" class="href-ref ref-defterm">Pauli matrix</a> (or a <a href="#defterm-Pauli_20Xmatrices" class="href-term">Pauli matrix</a>). Recall the definition of a <a href="#defterm-qu_5cX_28Xd_5cX_29Xit" class="href-term">qu<span class="inline-math">\(d\)</span>it</a>, noting that a qubit is also a <a href="#defterm-qu_5cX_28Xd_5cX_29Xit" class="href-term">qu<span class="inline-math">\(d=2\)</span>it</a>.</p>
<p><span class="verbatimtext">   Now \!this%$%^}] &lt;is&gt; some text that
   will be rendered
verbatim.
as \{is.\end{verbatim\ ha no, gotcha parser!
  </span></p>
<p><span class="textit">Vertaim macros like <span class="verbatimtext">}</span> and <span class="verbatimtext">{</span> should be parsed correctly, even as macro arguments!</span></p>
<h1 id="sec-test-label-another-sec" class="heading-level-1">Another section</h1>
<p>We can try to add some footnotes, too.<a href="#footnote-1" class="href-endnote endnote footnote">a</a> Does it work?</p>
<p>See also the topic <a href="#defterm-qu_5cX_28Xd_5cX_29Xit" class="href-ref ref-topic">qu<span class="inline-math">\(d\)</span>it</a>.</p>
<h1 class="heading-level-1">Footnotes</h1>
<dl class="enumeration footnote-list"><dt id="footnote-1">a</dt><dd>Like this! Does it work?</dd></dl>
""".strip()
        )



if __name__ == '__main__':
    unittest.main()
