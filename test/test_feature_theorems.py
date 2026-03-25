import unittest


from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer

from flm.feature import theorems as feature_theorems
from flm.feature import numbering as feature_numbering

from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer


def mk_flm_environ_wthms(**kwargs):
    features = standard_features()
    features.append( feature_theorems.FeatureTheorems(**kwargs) )
    return make_standard_environment(features)


def render_doc(environ, flm_input, fr=None):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    if fr is None:
        fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


# ------------------------------------------------------------------
# Init / Config tests
# ------------------------------------------------------------------

class TestFeatureTheoremsInit(unittest.TestCase):

    def test_feature_name(self):
        ft = feature_theorems.FeatureTheorems()
        self.assertEqual(ft.feature_name, 'theorems')

    def test_feature_dependencies(self):
        ft = feature_theorems.FeatureTheorems()
        self.assertEqual(ft.feature_dependencies, ['refs'])
        self.assertEqual(ft.feature_optional_dependencies, ['numbering'])

    def test_default_environments(self):
        ft = feature_theorems.FeatureTheorems()
        envs = sorted(ft.environments.keys())
        self.assertEqual(envs, [
            'conjecture', 'corollary', 'definition', 'lemma',
            'proof', 'proposition', 'remark', 'theorem',
        ])

    def test_simpleset_environments(self):
        ft = feature_theorems.FeatureTheorems(environments='simpleset')
        envs = sorted(ft.environments.keys())
        self.assertEqual(envs, [
            'corollary', 'definition', 'lemma', 'proof',
            'proposition', 'theorem',
        ])

    def test_richset_environments(self):
        ft = feature_theorems.FeatureTheorems(environments='richset')
        envs = sorted(ft.environments.keys())
        self.assertEqual(envs, [
            'claim', 'conjecture', 'corollary', 'definition', 'idea',
            'lemma', 'observation', 'problem', 'proof', 'proposition',
            'question', 'remark', 'theorem',
        ])

    def test_title_standardization_lowercase_string(self):
        ft = feature_theorems.FeatureTheorems()
        title = ft.environments['theorem']['title']
        self.assertEqual(title['lowercase'], {'singular': 'theorem', 'plural': 'theorems'})
        self.assertEqual(title['capital'], {'singular': 'Theorem', 'plural': 'Theorems'})
        self.assertEqual(title['abbreviated'], {'singular': 'Thm.', 'plural': 'Thms.'})

    def test_title_standardization_custom(self):
        ft = feature_theorems.FeatureTheorems(
            environments={'theoremlike': {'mytheorem': {'title': 'my theorem'}}},
        )
        title = ft.environments['mytheorem']['title']
        self.assertEqual(title['lowercase'], {'singular': 'my theorem', 'plural': 'my theorems'})
        self.assertEqual(title['capital'], {'singular': 'My theorem', 'plural': 'My theorems'})

    def test_title_standardization_string_spec(self):
        """Environment spec given as plain string instead of dict."""
        ft = feature_theorems.FeatureTheorems(
            environments={'theoremlike': {'mytheorem': 'my theorem'}},
        )
        title = ft.environments['mytheorem']['title']
        self.assertEqual(title['lowercase'], {'singular': 'my theorem', 'plural': 'my theorems'})

    def test_default_allowed_ref_label_prefixes(self):
        ft = feature_theorems.FeatureTheorems()
        self.assertEqual(ft.allowed_ref_label_prefixes, [
            'thm', 'prop', 'cor', 'lem', 'rem', 'def', 'dfn', 'x', 'topic'
        ])

    def test_custom_allowed_ref_label_prefixes(self):
        ft = feature_theorems.FeatureTheorems(allowed_ref_label_prefixes=['thm', 'lem'])
        self.assertEqual(ft.allowed_ref_label_prefixes, ['thm', 'lem'])

    def test_add_latex_context_definitions(self):
        ft = feature_theorems.FeatureTheorems()
        defs = ft.add_latex_context_definitions()
        env_names = sorted([e.environmentname for e in defs['environments']])
        self.assertEqual(env_names, [
            'conjecture', 'corollary', 'definition', 'lemma',
            'proof', 'proposition', 'remark', 'theorem',
        ])
        self.assertFalse('macros' in defs)

    def test_proof_title(self):
        ft = feature_theorems.FeatureTheorems()
        title = ft.environments['proof']['title']
        self.assertEqual(title['lowercase'], {'singular': 'proof', 'plural': 'proofs'})
        self.assertEqual(title['capital'], {'singular': 'Proof', 'plural': 'Proofs'})
        self.assertEqual(title['abbreviated'], {'singular': 'Proof', 'plural': 'Proofs'})

    def test_remark_title(self):
        ft = feature_theorems.FeatureTheorems()
        title = ft.environments['remark']['title']
        self.assertEqual(title['lowercase'], {'singular': 'remark', 'plural': 'remarks'})
        self.assertEqual(title['capital'], {'singular': 'Remark', 'plural': 'Remarks'})
        self.assertEqual(title['abbreviated'], {'singular': 'Rem.', 'plural': 'Rems.'})


# ------------------------------------------------------------------
# HTML Rendering tests
# ------------------------------------------------------------------

class TestFeatureTheoremsHtml(unittest.TestCase):

    maxDiff = None

    def test_theorem_with_title_and_proof(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}[My theorem]
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{proof}
We argue by contradiction.
\end{proof}""", fr=HtmlFragmentRenderer())
        self.assertEqual(
            result, r"""
<div class="theoremlike theorem p-block"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;1 (My theorem)</span> The square root of two is irrational.</p></div>
<div class="prooflike proof p-block"><p><span id="sec--" class="heading-level-theorem heading-inline">Proof</span> We argue by contradiction.&nbsp;□</p></div>
""".strip()
        )

    def test_simple_1(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{theorem}The square root of two is irrational.\end{theorem}')
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>'
        )

    def test_simple_1b_multiline(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}
The square root of two is irrational.
\end{theorem}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>'
        )

    def test_theorem_with_title(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}[Irrationality of the square root of two]
The square root of two is irrational.
\end{theorem}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1 (Irrationality of the square root of two)</span>'
            ' The square root of two is irrational.</p></div>'
        )

    def test_shared_counter_simple(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}
The square root of two is irrational.
\end{theorem}

\begin{lemma}
The square root of three is irrational.
\end{lemma}

\begin{proposition}
The square root of four is rational.
\end{proposition}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>\n'
            '<div class="theoremlike lemma p-block"><p>'
            '<span id="lemma-2" class="heading-level-theorem heading-inline">'
            'Lemma&nbsp;2</span> The square root of three is irrational.</p></div>\n'
            '<div class="theoremlike proposition p-block"><p>'
            '<span id="proposition-3" class="heading-level-theorem heading-inline">'
            'Proposition&nbsp;3</span> The square root of four is rational.</p></div>'
        )

    def test_separate_counters_simple(self):
        environ = mk_flm_environ_wthms(
            theorem_types={
                'theoremlike': {
                    'shared_numbering': False,
                    'counter_formatter': 'Roman',
                },
                'lemmaandproplike': {
                    'shared_numbering': True,
                },
            },
            environments={
                'theoremlike': {
                    'theorem': {'title': 'theorem'},
                },
                'lemmaandproplike': {
                    'proposition': {'title': 'proposition'},
                    'lemma': {'title': 'lemma'},
                },
            },
            shared_counter_formatter='alph',
        )
        result = render_doc(environ, r"""
\begin{theorem}
The square root of two is irrational.
\end{theorem}

\begin{lemma}
The square root of three is irrational.
\end{lemma}

\begin{proposition}
The square root of four is rational.
\end{proposition}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;I</span> The square root of two is irrational.</p></div>\n'
            '<div class="lemmaandproplike lemma p-block"><p>'
            '<span id="lemma-1" class="heading-level-theorem heading-inline">'
            'Lemma&nbsp;a</span> The square root of three is irrational.</p></div>\n'
            '<div class="lemmaandproplike proposition p-block"><p>'
            '<span id="proposition-2" class="heading-level-theorem heading-inline">'
            'Proposition&nbsp;b</span> The square root of four is rational.</p></div>'
        )

    def test_ref_multi(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{theorem}
\label{thm:sqrt3}
The square root of three is irrational.
\end{theorem}

\begin{proposition}
\label{thm:sqrt4}
The square root of four is rational.
\end{proposition}

Ref: \ref{thm:sqrt2,thm:sqrt4,thm:sqrt3}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>\n'
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-2" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;2</span> The square root of three is irrational.</p></div>\n'
            '<div class="theoremlike proposition p-block"><p>'
            '<span id="proposition-3" class="heading-level-theorem heading-inline">'
            'Proposition&nbsp;3</span> The square root of four is rational.</p></div>\n'
            '<p>Ref: <a href="#theorem-1" class="href-ref refcnt-theorem">Theorems&nbsp;</a>'
            '<a href="#theorem-1" class="href-ref refcnt-theorem">1</a>'
            ' and&nbsp;<a href="#theorem-2" class="href-ref refcnt-theorem">2</a>, '
            '<a href="#proposition-3" class="href-ref refcnt-proposition">'
            'Proposition&nbsp;3</a></p>'
        )

    def test_numbering_by_section(self):
        features = standard_features(heading_numbering_section_depth=1)
        features.append(feature_numbering.FeatureNumbering(
            number_within={
                '_theorems_shared': {'reset_at': 'section', 'numprefix': '${section}.'},
            }
        ))
        features.append(feature_theorems.FeatureTheorems())
        environ = make_standard_environment(features)
        result = render_doc(environ, r"""
\section{One}
\begin{theorem}
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\section{Two}
\begin{theorem}
\label{thm:sqrt3}
The square root of three is irrational.
\end{theorem}

\begin{proposition}
\label{thm:sqrt4}
The square root of four is rational.
\end{proposition}

Ref: \ref{thm:sqrt2,thm:sqrt4,thm:sqrt3}""")
        self.assertEqual(
            result,
            '<h1 id="sec--One" class="heading-level-1">1. One</h1>\n'
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1.1</span> The square root of two is irrational.</p></div>\n'
            '<h1 id="sec--Two" class="heading-level-1">2. Two</h1>\n'
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-2-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;2.1</span> The square root of three is irrational.</p></div>\n'
            '<div class="theoremlike proposition p-block"><p>'
            '<span id="proposition-2-2" class="heading-level-theorem heading-inline">'
            'Proposition&nbsp;2.2</span> The square root of four is rational.</p></div>\n'
            '<p>Ref: <a href="#theorem-1-1" class="href-ref refcnt-theorem">Theorems&nbsp;</a>'
            '<a href="#theorem-1-1" class="href-ref refcnt-theorem">1.1</a>'
            ' and&nbsp;<a href="#theorem-2-1" class="href-ref refcnt-theorem">2.1</a>, '
            '<a href="#proposition-2-2" class="href-ref refcnt-proposition">'
            'Proposition&nbsp;2.2</a></p>'
        )

    def test_definition_environment(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{definition}A number is \emph{rational} if it can be expressed as a fraction.\end{definition}')
        self.assertEqual(
            result,
            '<div class="definitionlike definition p-block"><p>'
            '<span id="definition-1" class="heading-level-theorem heading-inline">'
            'Definition&nbsp;1</span> A number is '
            '<span class="textit">rational</span>'
            ' if it can be expressed as a fraction.</p></div>'
        )

    def test_remark_environment(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{remark}This is a remark.\end{remark}')
        self.assertEqual(
            result,
            '<div class="definitionlike remark p-block"><p>'
            '<span id="remark-1" class="heading-level-theorem heading-inline">'
            'Remark&nbsp;1</span> This is a remark.</p></div>'
        )

    def test_conjecture_environment(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{conjecture}This is a conjecture.\end{conjecture}')
        self.assertEqual(
            result,
            '<div class="theoremlike conjecture p-block"><p>'
            '<span id="conjecture-1" class="heading-level-theorem heading-inline">'
            'Conjecture&nbsp;1</span> This is a conjecture.</p></div>'
        )

    def test_proof_environment(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{proof}We argue by contradiction.\end{proof}')
        self.assertEqual(
            result,
            '<div class="prooflike proof p-block"><p>'
            '<span id="sec--" class="heading-level-theorem heading-inline">'
            'Proof</span> We argue by contradiction.&nbsp;\u25a1</p></div>'
        )

    def test_proof_with_custom_title(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{proof}[Sketch of proof]We argue by contradiction.\end{proof}')
        self.assertEqual(
            result,
            '<div class="prooflike proof p-block"><p>'
            '<span id="sec--Sketch-of-proof" class="heading-level-theorem heading-inline">'
            'Proof (Sketch of proof)</span> We argue by contradiction.&nbsp;\u25a1</p></div>'
        )

    def test_proof_with_relation_ref(self):
        """Proof with *label shows 'Proof (Theorem N)'."""
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{proof}[*thm:sqrt2]
We argue by contradiction.
\end{proof}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>\n'
            '<div class="prooflike proof p-block"><p>'
            '<span id="sec--Theorem-1" class="heading-level-theorem heading-inline">'
            'Proof (Theorem&nbsp;1)</span> We argue by contradiction.&nbsp;\u25a1</p></div>'
        )

    def test_proof_with_double_star_ref(self):
        """Proof with **label does NOT show the ref in the heading."""
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{proof}[**thm:sqrt2]
We argue by contradiction.
\end{proof}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>\n'
            '<div class="prooflike proof p-block"><p>'
            '<span id="sec--" class="heading-level-theorem heading-inline">'
            'Proof</span> We argue by contradiction.&nbsp;\u25a1</p></div>'
        )

    def test_noproofref_html(self):
        """\\noproofref does not affect HTML rendering (only metadata)."""
        environ = mk_flm_environ_wthms()
        result = render_doc(environ,
            r'\begin{theorem}\noproofref The square root of two is irrational.\end{theorem}')
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> The square root of two is irrational.</p></div>'
        )

    def test_shared_counter_across_types(self):
        """Theoremlike + definitionlike share the same counter by default."""
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}First theorem.\end{theorem}
\begin{definition}First definition.\end{definition}
\begin{remark}First remark.\end{remark}
\begin{theorem}Second theorem.\end{theorem}""")
        self.assertEqual(
            result,
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-1" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;1</span> First theorem.</p></div>\n'
            '<div class="definitionlike definition p-block"><p>'
            '<span id="definition-2" class="heading-level-theorem heading-inline">'
            'Definition&nbsp;2</span> First definition.</p></div>\n'
            '<div class="definitionlike remark p-block"><p>'
            '<span id="remark-3" class="heading-level-theorem heading-inline">'
            'Remark&nbsp;3</span> First remark.</p></div>\n'
            '<div class="theoremlike theorem p-block"><p>'
            '<span id="theorem-4" class="heading-level-theorem heading-inline">'
            'Theorem&nbsp;4</span> Second theorem.</p></div>'
        )


# ------------------------------------------------------------------
# Text renderer tests
# ------------------------------------------------------------------

class TestFeatureTheoremsText(unittest.TestCase):

    maxDiff = None

    def test_theorem_with_title_and_proof(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}[My theorem]
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{proof}
We argue by contradiction.
\end{proof}""", fr=TextFragmentRenderer())
        self.assertEqual(
            result,
            'Theorem\xa01 (My theorem).  The square root of two is irrational.'
            '\n\nProof.  We argue by contradiction.\xa0\u25a1'
        )


# ------------------------------------------------------------------
# LaTeX renderer tests
# ------------------------------------------------------------------

class TestFeatureTheoremsLatex(unittest.TestCase):

    maxDiff = None

    def test_theorem_with_title_and_proof(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}[My theorem]
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{proof}
We argue by contradiction.
\end{proof}""", fr=LatexFragmentRenderer())
        self.assertEqual(
            result,
            '\\begin{flmThmTheoremLike}\\flmTheoremHeading{Theorem~1 (My theorem)}%\n'
            '\\label{x:theorem-1}%\n'
            'The square root of two is irrational.\n'
            '\\end{flmThmTheoremLike}%\n\n'
            '\\begin{flmThmProofLike}\\flmTheoremHeading{Proof}%\n'
            '\\label{x:sec--}%\n'
            'We argue by contradiction.~\\ensuremath{\\square}\n'
            '\\end{flmThmProofLike}%\n'
        )


# ------------------------------------------------------------------
# Markdown renderer tests
# ------------------------------------------------------------------

class TestFeatureTheoremsMarkdown(unittest.TestCase):

    maxDiff = None

    def test_theorem_with_title_and_proof(self):
        environ = mk_flm_environ_wthms()
        result = render_doc(environ, r"""
\begin{theorem}[My theorem]
\label{thm:sqrt2}
The square root of two is irrational.
\end{theorem}

\begin{proof}
We argue by contradiction.
\end{proof}""", fr=MarkdownFragmentRenderer())
        self.assertEqual(
            result,
            '<a name="theorem-1"></a> Theorem\xa01 \\(My theorem\\).  \n'
            'The square root of two is irrational\\.\n\n'
            '<a name="sec--"></a> Proof.  \n'
            'We argue by contradiction\\.\xa0\u25a1'
        )


# ------------------------------------------------------------------
# Recomposer tests
# ------------------------------------------------------------------

class TestFeatureTheoremsRecomposer(unittest.TestCase):

    maxDiff = None

    def test_simple_theorem(self):
        environ = mk_flm_environ_wthms()
        src = r'\begin{theorem}The square root of two is irrational.\end{theorem}'
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{theorem}The square root of two is irrational.\end{theorem}'
        )

    def test_theorem_with_title(self):
        environ = mk_flm_environ_wthms()
        src = r'\begin{theorem}[My Theorem]The square root of two is irrational.\end{theorem}'
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{theorem}[{My Theorem}]The square root of two is irrational.\end{theorem}'
        )

    def test_theorem_with_label(self):
        environ = mk_flm_environ_wthms()
        src = r'\begin{theorem}\label{thm:sqrt2}The square root of two is irrational.\end{theorem}'
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{theorem}\label{ref1}The square root of two is irrational.\end{theorem}'
        )

    def test_proof(self):
        environ = mk_flm_environ_wthms()
        src = r'\begin{proof}We argue by contradiction.\end{proof}'
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{proof}We argue by contradiction.\end{proof}'
        )

    def test_proof_with_relation_ref(self):
        environ = mk_flm_environ_wthms()
        src = (
            r'\begin{theorem}' '\n'
            r'\label{thm:sqrt2}' '\n'
            'The square root of two is irrational.\n'
            r'\end{theorem}' '\n\n'
            r'\begin{proof}[*thm:sqrt2]' '\n'
            'We argue by contradiction.\n'
            r'\end{proof}'
        )
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            '\\begin{theorem}\\label{ref1}\n'
            'The square root of two is irrational.\n'
            '\\end{theorem}\n\n'
            '\\begin{proof}[*ref1]\n'
            'We argue by contradiction.\n'
            '\\end{proof}'
        )

    def test_proof_with_double_star_ref(self):
        environ = mk_flm_environ_wthms()
        src = (
            r'\begin{theorem}' '\n'
            r'\label{thm:sqrt2}' '\n'
            'The square root of two is irrational.\n'
            r'\end{theorem}' '\n\n'
            r'\begin{proof}[**thm:sqrt2]' '\n'
            'We argue by contradiction.\n'
            r'\end{proof}'
        )
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            '\\begin{theorem}\\label{ref1}\n'
            'The square root of two is irrational.\n'
            '\\end{theorem}\n\n'
            '\\begin{proof}[**ref1]\n'
            'We argue by contradiction.\n'
            '\\end{proof}'
        )

    def test_noproofref(self):
        environ = mk_flm_environ_wthms()
        src = r'\begin{theorem}\noproofref The square root of two is irrational.\end{theorem}'
        frag = environ.make_fragment(src)
        recomposer = FLMPureLatexRecomposer({})
        result = recomposer.recompose_pure_latex(frag.nodes)
        self.assertEqual(
            result['latex'],
            r'\begin{theorem}\noproofref The square root of two is irrational.\end{theorem}'
        )


# ------------------------------------------------------------------
# Error cases
# ------------------------------------------------------------------

class TestFeatureTheoremsErrors(unittest.TestCase):

    def test_label_on_unnumbered_proof_raises(self):
        environ = mk_flm_environ_wthms()
        with self.assertRaises(Exception):
            render_doc(environ,
                r'\begin{proof}\label{thm:pf1}Something.\end{proof}')


if __name__ == '__main__':
    import logging
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
