import unittest

from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.stdfeatures import standard_features
from flm.flmenvironment import make_standard_environment
from flm.flmrendercontext import FLMStandaloneModeRenderContext


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


class TestLatexFragmentRenderer(unittest.TestCase):

    def test_esc_00(self):
        lfr = LatexFragmentRenderer()

        self.assertEqual(
            lfr.latexescape('{x}'),
            r'\{x\}'
        )

    def test_text_format_0(self):
        lfr = LatexFragmentRenderer()

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"Text content"
        )

        render_result = lfr.render_text_format(
            ['textbf'],
            frag1.nodes,
            FLMStandaloneModeRenderContext(lfr),
        )
        self.assertEqual(
            render_result.replace('%\n', '').replace(r'\relax ', '') .strip(),
            r"""
\textbf{Text content}
""" .strip()
        )


    def test_delayed_rendering(self):
        lfr = LatexFragmentRenderer()

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"""
\section{My Section}
\label{sec:mysec}
Hello.

See \ref{sec:mysec}.
""" .strip()
        )

        def render_doc_fn(render_context):
            return { 'content': frag1.render(render_context) }

        doc = environ.make_document(render_doc_fn)
        result, _ = doc.render(lfr)

        self.assertEqual(
            result['content'].replace('%\n', '').replace(r'\relax ', '') .strip(),
            r"""
\section{My Section}\label{x:sec-mysec}
Hello.

See \hyperref[{x:sec-mysec}]{My Section}.
            """ .strip()
        )

        
        
    
    def test_simple_00(self):
        
        fr = LatexFragmentRenderer()

        fr.text_format_cmds['defterm-term'] = 'ecztermdef'
        fr.latex_wrap_verbatim_macro = 'eczshowverbatim'
        fr.latex_semantic_block_environments['figure_caption'] = 'eczfigcaption'


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

        def render_doc_fn(render_context):
            return frag1.render(render_context)

        doc = environ.make_document(render_doc_fn)
        result, _ = doc.render(fr)

        self.assertEqual(
            result.replace('%\n', '').replace(r'\relax ', '') .strip(),
            r"""
\begin{enumerate}% enumeration,enumerate
\item[{1.}]One
\item[{2.}]Two
\item[{c}]Three\end{enumerate}
            """ .strip()
        )


    def test_enumerate_00(self):
        lfr = LatexFragmentRenderer()

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(
            r"1."
        )
        frag2 = environ.make_fragment(
            r"Hello."
        )

        render_result = lfr.render_enumeration(
            [ frag2.nodes ],
            lambda n: frag1.nodes,
            FLMStandaloneModeRenderContext(lfr),
        )
        self.assertEqual(
            render_result.replace('%\n', '').replace(r'\relax ', ''),
            r"""
\begin{itemize}% enumeration
\item[{1.}]Hello.\end{itemize}
""" .strip()
        )

        


if __name__ == '__main__':
    unittest.main()
