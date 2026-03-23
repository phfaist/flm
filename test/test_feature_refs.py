import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer

from flm.feature.refs import (
    FeatureRefs,
    RefInstance,
    ReferenceableInfo,
    get_safe_target_id,
)
from flm.feature.headings import FeatureHeadings
from flm.feature.numbering import FeatureNumbering


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


def mk_flm_environ_numbered_headings(**kwargs):
    features = standard_features(headings=False, **kwargs)
    features.insert(0, FeatureHeadings(numbering_section_depth=2))
    features.append(FeatureNumbering())
    return make_standard_environment(features)


def render_doc(environ, flm_input):
    frag = environ.make_fragment(flm_input.strip())
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, _ = doc.render(fr)
    return result


# -----------------------------------------------------------------------
# Utilities
# -----------------------------------------------------------------------

class TestGetSafeTargetId(unittest.TestCase):

    def test_simple_alphanumeric(self):
        self.assertEqual(get_safe_target_id('eq', 'abc'), 'eq-abc')

    def test_colon_in_label(self):
        # colon ord is 0x3a
        self.assertEqual(
            get_safe_target_id('eq', 'my:label'),
            'eq-my_3aXlabel'
        )

    def test_space_in_label(self):
        # space ord is 0x20
        self.assertEqual(
            get_safe_target_id('sec', 'my section'),
            'sec-my_20Xsection'
        )

    def test_hyphen_preserved(self):
        self.assertEqual(get_safe_target_id('sec', 'my-label'), 'sec-my-label')

    def test_special_characters(self):
        # '/' ord is 0x2f
        self.assertEqual(
            get_safe_target_id('type', 'a/b'),
            'type-a_2fXb'
        )


class TestReferenceableInfo(unittest.TestCase):

    def test_basic_construction(self):
        ri = ReferenceableInfo(
            kind='heading',
            formatted_ref_flm_text='My Heading',
            labels=[('sec', 'intro')]
        )
        self.assertEqual(ri.kind, 'heading')
        self.assertEqual(ri.formatted_ref_flm_text, 'My Heading')
        self.assertEqual(ri.labels, [('sec', 'intro')])

    def test_get_target_id(self):
        ri = ReferenceableInfo(
            kind='heading',
            formatted_ref_flm_text='Intro',
            labels=[('sec', 'intro')]
        )
        self.assertEqual(ri.get_target_id(), 'sec-intro')

    def test_get_target_id_no_labels(self):
        ri = ReferenceableInfo(
            kind='heading',
            formatted_ref_flm_text='Intro',
            labels=[]
        )
        self.assertIsNone(ri.get_target_id())

    def test_multiple_labels_uses_first(self):
        ri = ReferenceableInfo(
            kind='heading',
            formatted_ref_flm_text='Intro',
            labels=[('sec', 'first'), ('sec', 'second')]
        )
        self.assertEqual(ri.get_target_id(), 'sec-first')

    def test_asdict(self):
        ri = ReferenceableInfo(
            kind='heading',
            formatted_ref_flm_text='Intro',
            labels=[('sec', 'intro')]
        )
        d = ri.asdict()
        self.assertEqual(d['kind'], 'heading')
        self.assertEqual(d['formatted_ref_flm_text'], 'Intro')
        self.assertEqual(d['labels'], [('sec', 'intro')])


class TestRefInstance(unittest.TestCase):

    def test_basic_construction(self):
        ri = RefInstance(
            ref_type='eq',
            ref_label='a',
            formatted_ref_flm_text='(1)',
            target_href='#equation-1',
            counter_value=None,
            counter_numprefix=None,
            counter_formatter_id=None,
        )
        self.assertEqual(ri.ref_type, 'eq')
        self.assertEqual(ri.ref_label, 'a')
        self.assertEqual(ri.target_href, '#equation-1')

    def test_asdict(self):
        ri = RefInstance(
            ref_type='eq',
            ref_label='a',
            formatted_ref_flm_text='(1)',
            target_href='#equation-1',
            counter_value=None,
            counter_numprefix=None,
            counter_formatter_id=None,
        )
        d = ri.asdict()
        self.assertEqual(d['ref_type'], 'eq')
        self.assertEqual(d['ref_label'], 'a')
        self.assertEqual(d['formatted_ref_flm_text'], '(1)')

    def test_repr(self):
        ri = RefInstance(
            ref_type='eq',
            ref_label='a',
            formatted_ref_flm_text='(1)',
            target_href='#equation-1',
            counter_value=None,
            counter_numprefix=None,
            counter_formatter_id=None,
        )
        r = repr(ri)
        self.assertTrue('RefInstance' in r)
        self.assertTrue('eq' in r)


# -----------------------------------------------------------------------
# \ref with equation labels
# -----------------------------------------------------------------------

class TestRefWithEquations(unittest.TestCase):

    maxDiff = None

    def test_ref_equation_with_prefix(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:a}
\end{align}

See \ref{eq:a}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:a}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p>See '
            '<a href="#equation-1" class="href-ref ref-eq">Eq.&nbsp;(1)</a>'
            '.</p>'
        )

    def test_ref_equation_capital_variant(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:a}
\end{align}

See \ref[S]{eq:a}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:a}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p>See '
            '<a href="#equation-1" class="href-ref ref-eq">Equation&nbsp;(1)</a>'
            '.</p>'
        )

    def test_eqref_no_prefix(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:a}
\end{align}

See~\eqref{eq:a}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:a}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p>See&nbsp;'
            '<a href="#equation-1" class="href-ref ref-eq">(1)</a>'
            '.</p>'
        )

    def test_ref_and_eqref_together(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:a}
\end{align}

See \ref{eq:a}.

See \ref[S]{eq:a}.

See~\eqref{eq:a}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:a}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p>See <a href="#equation-1" class="href-ref ref-eq">'
            'Eq.&nbsp;(1)</a>.</p>\n'
            '<p>See <a href="#equation-1" class="href-ref ref-eq">'
            'Equation&nbsp;(1)</a>.</p>\n'
            '<p>See&nbsp;<a href="#equation-1" class="href-ref ref-eq">'
            '(1)</a>.</p>'
        )

    def test_multiple_equations_separate_labels(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:first}
\end{align}

\begin{align}
  y=2
  \label{eq:second}
\end{align}

First: \ref{eq:first}. Second: \ref{eq:second}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:first}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p><span id="equation-2" class="display-math env-align">'
            r'\begin{align}' '\n  y=2\n  ' r'\label{eq:second}' '\n'
            r'\tag*{(2)}\end{align}'
            '</span></p>\n'
            '<p>First: '
            '<a href="#equation-1" class="href-ref ref-eq">Eq.&nbsp;(1)</a>'
            '. Second: '
            '<a href="#equation-2" class="href-ref ref-eq">Eq.&nbsp;(2)</a>'
            '.</p>'
        )

    def test_equation_environment(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{equation}
  E=mc^2
  \label{eq:einstein}
\end{equation}

See \ref{eq:einstein}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-equation">'
            r'\begin{equation}' '\n  E=mc^2\n  ' r'\label{eq:einstein}' '\n'
            r'\tag*{(1)}\end{equation}'
            '</span></p>\n'
            '<p>See '
            '<a href="#equation-1" class="href-ref ref-eq">Eq.&nbsp;(1)</a>'
            '.</p>'
        )

    def test_gather_environment(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{gather}
  a+b=c
  \label{eq:sum}
\end{gather}

See \ref{eq:sum}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-gather">'
            r'\begin{gather}' '\n  a+b=c\n  ' r'\label{eq:sum}' '\n'
            r'\tag*{(1)}\end{gather}'
            '</span></p>\n'
            '<p>See '
            '<a href="#equation-1" class="href-ref ref-eq">Eq.&nbsp;(1)</a>'
            '.</p>'
        )


# -----------------------------------------------------------------------
# \ref with heading labels
# -----------------------------------------------------------------------

class TestRefWithHeadings(unittest.TestCase):

    maxDiff = None

    def test_ref_unnumbered_section(self):
        environ = mk_flm_environ()
        result = render_doc(
            environ,
            r'\section{Methods}\label{sec:methods} See \ref{sec:methods}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-methods" class="heading-level-1">Methods</h1>\n'
            '<p>See '
            '<a href="#sec-methods" class="href-ref ref-sec">Methods</a>'
            '.</p>'
        )

    def test_ref_numbered_section(self):
        environ = mk_flm_environ_numbered_headings()
        result = render_doc(
            environ,
            r'\section{Intro}\label{sec:intro} See \ref{sec:intro}.'
        )
        self.assertEqual(
            result,
            '<h1 id="sec-intro" class="heading-level-1">1. Intro</h1>\n'
            '<p>See '
            '<a href="#sec-intro" class="href-ref ref-sec">'
            '§&thinsp;1</a>'
            '.</p>'
        )

    def test_ref_forward_reference(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
See \ref{sec:later}.

\section{Later Section}\label{sec:later}
""")
        self.assertEqual(
            result,
            '<p>See '
            '<a href="#sec-later" class="href-ref ref-sec">Later Section</a>'
            '.</p>\n'
            '<h1 id="sec-later" class="heading-level-1">Later Section</h1>'
        )


# -----------------------------------------------------------------------
# \hyperref
# -----------------------------------------------------------------------

class TestHyperref(unittest.TestCase):

    maxDiff = None

    def test_hyperref_custom_text(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\section{Methods}\label{sec:methods}

See \hyperref[sec:methods]{the methods section}.
""")
        self.assertEqual(
            result,
            '<h1 id="sec-methods" class="heading-level-1">Methods</h1>\n'
            '<p>See '
            '<a href="#sec-methods" class="href-ref ref-sec">'
            'the methods section</a>'
            '.</p>'
        )

    def test_hyperref_with_formatting(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\section{Methods}\label{sec:methods}

See \hyperref[sec:methods]{\textbf{methods}}.
""")
        self.assertEqual(
            result,
            '<h1 id="sec-methods" class="heading-level-1">Methods</h1>\n'
            '<p>See '
            '<a href="#sec-methods" class="href-ref ref-sec">'
            '<span class="textbf">methods</span></a>'
            '.</p>'
        )

    def test_hyperref_equation(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:main}
\end{align}

See \hyperref[eq:main]{the main equation}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:main}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p>See '
            '<a href="#equation-1" class="href-ref ref-eq">'
            'the main equation</a>'
            '.</p>'
        )


# -----------------------------------------------------------------------
# Multiple refs (comma-separated)
# -----------------------------------------------------------------------

class TestRefMany(unittest.TestCase):

    maxDiff = None

    def test_ref_many_equations(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  x=1
  \label{eq:a}
\end{align}

\begin{align}
  y=2
  \label{eq:b}
\end{align}

See \ref{eq:a,eq:b}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  x=1\n  ' r'\label{eq:a}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p><span id="equation-2" class="display-math env-align">'
            r'\begin{align}' '\n  y=2\n  ' r'\label{eq:b}' '\n'
            r'\tag*{(2)}\end{align}'
            '</span></p>\n'
            '<p>See '
            '<a href="#equation-1" class="href-ref refcnt-eq">Eqs.&nbsp;</a>'
            '(<a href="#equation-1" class="href-ref refcnt-eq">1</a>'
            ',<a href="#equation-2" class="href-ref refcnt-eq">2</a>)'
            '.</p>'
        )

    def test_ref_many_numbered_sections(self):
        environ = mk_flm_environ_numbered_headings()
        result = render_doc(environ, r"""
\section{Alpha}\label{sec:alpha}
\section{Beta}\label{sec:beta}

See \ref{sec:alpha,sec:beta}.
""")
        self.assertEqual(
            result,
            '<h1 id="sec-alpha" class="heading-level-1">1. Alpha</h1>\n'
            '<h1 id="sec-beta" class="heading-level-1">2. Beta</h1>\n'
            '<p>See '
            '<a href="#sec-alpha" class="href-ref refcnt-section">'
            '§&thinsp;</a>'
            '<a href="#sec-alpha" class="href-ref refcnt-section">1</a>'
            ','
            '<a href="#sec-beta" class="href-ref refcnt-section">2</a>'
            '.</p>'
        )


# -----------------------------------------------------------------------
# External ref resolvers
# -----------------------------------------------------------------------

class TestExternalRefResolver(unittest.TestCase):

    maxDiff = None

    def _make_resolver(self):
        class MyRefResolver:
            def get_ref(self, ref_type, ref_label, resource_info,
                        render_context):
                if ref_type == 'code' and ref_label == 'surface':
                    return RefInstance(
                        ref_type='code',
                        ref_label='surface',
                        formatted_ref_flm_text=r'Kitaev \emph{surface} code',
                        target_href='https://example.com/surface',
                        counter_value=None,
                        counter_numprefix=None,
                        counter_formatter_id=None,
                    )
                raise ValueError(
                    f"Unknown ref '{ref_type}:{ref_label}'"
                )
        return MyRefResolver()

    def test_external_ref_resolved(self):
        resolver = self._make_resolver()
        environ = mk_flm_environ(external_ref_resolvers=[resolver])
        result = render_doc(environ, r'See \ref{code:surface}.')
        self.assertEqual(
            result,
            'See '
            '<a href="https://example.com/surface" class="href-ref ref-code">'
            'Kitaev <span class="textit">surface</span> code</a>.'
        )

    def test_external_ref_unresolved_raises(self):
        resolver = self._make_resolver()
        environ = mk_flm_environ(external_ref_resolvers=[resolver])
        with self.assertRaises(LatexWalkerLocatedError):
            render_doc(environ, r'See \ref{code:nonexistent}.')


# -----------------------------------------------------------------------
# Unresolved refs
# -----------------------------------------------------------------------

class TestUnresolvedRefs(unittest.TestCase):

    maxDiff = None

    def test_unresolved_ref_raises_by_default(self):
        environ = mk_flm_environ()
        with self.assertRaises(LatexWalkerLocatedError):
            render_doc(environ, r'See \ref{eq:missing}.')

    def test_allow_unresolved_refs(self):
        features = standard_features(refs=False)
        features.append(FeatureRefs(allow_unresolved_refs=True))
        environ = make_standard_environment(features)
        result = render_doc(environ, r'See \ref{eq:missing}.')
        self.assertEqual(
            result,
            'See '
            '<a href="javascript:alert(&quot;Unresolved reference!&quot;)"'
            ' class="href-ref ref-eq">'
            '<span class="textbf">&lt;'
            '<span class="verbatimtext verbatimtext-environment">'
            'eq:missing</span>'
            '&gt;</span></a>.'
        )

    def test_allow_unresolved_refs_with_custom_display(self):
        features = standard_features(refs=False)
        features.append(FeatureRefs(
            allow_unresolved_refs={
                'display_unresolved': lambda rt, rl:
                    f'[{rt}:{rl}]' if rt else f'[{rl}]'
            }
        ))
        environ = make_standard_environment(features)
        result = render_doc(environ, r'See \ref{eq:missing}.')
        self.assertEqual(
            result,
            'See '
            '<a href="javascript:alert(&quot;Unresolved reference!&quot;)"'
            ' class="href-ref ref-eq">'
            '[eq:missing]</a>.'
        )


# -----------------------------------------------------------------------
# Duplicate labels
# -----------------------------------------------------------------------

class TestDuplicateLabels(unittest.TestCase):

    maxDiff = None

    def test_duplicate_label_raises(self):
        environ = mk_flm_environ()
        with self.assertRaises(Exception):
            render_doc(environ, r"""
\begin{align}
  x=1 \label{eq:same}
\end{align}

\begin{align}
  y=2 \label{eq:same}
\end{align}
""")


# -----------------------------------------------------------------------
# Recomposer (pure LaTeX)
# -----------------------------------------------------------------------

class TestRefsRecomposer(unittest.TestCase):

    maxDiff = None

    def _recompose(self, flm_input, options=None):
        environ = mk_flm_environ()
        frag = environ.make_fragment(flm_input.strip())
        recomposer = FLMPureLatexRecomposer(options if options else {})
        result = recomposer.recompose_pure_latex(frag.nodes)
        return result['latex']

    def test_ref_recomposes_to_zcref(self):
        result = self._recompose(r'\ref{eq:abc}')
        self.assertEqual(
            result,
            r'\NoCaseChange{\protect\zcref{ref1}}'
        )

    def test_hyperref_recomposes_to_hyperref(self):
        result = self._recompose(r'\hyperref[eq:abc]{my text}')
        self.assertEqual(
            result,
            r'\NoCaseChange{\protect\hyperref[{ref1}]{my text}}'
        )

    def test_ref_with_emit_flm_macro(self):
        result = self._recompose(
            r'\ref{eq:abc}',
            options={'refs': {'emit_flm_macro': True}}
        )
        self.assertEqual(result, r'\flmRefsCref{ref1}')

    def test_hyperref_with_emit_flm_macro(self):
        result = self._recompose(
            r'\hyperref[eq:abc]{my text}',
            options={'refs': {'emit_flm_macro': True}}
        )
        self.assertEqual(result, r'\flmRefsHyperref{ref1}{my text}')


# -----------------------------------------------------------------------
# Edge cases
# -----------------------------------------------------------------------

class TestRefEdgeCases(unittest.TestCase):

    maxDiff = None

    def test_ref_with_tilde_nonbreaking_space(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\section{Methods}\label{sec:methods}

See~\ref{sec:methods}.
""")
        self.assertEqual(
            result,
            '<h1 id="sec-methods" class="heading-level-1">Methods</h1>\n'
            '<p>See&nbsp;'
            '<a href="#sec-methods" class="href-ref ref-sec">Methods</a>'
            '.</p>'
        )

    def test_equation_numbering_sequential(self):
        environ = mk_flm_environ()
        result = render_doc(environ, r"""
\begin{align}
  a=1 \label{eq:a}
\end{align}

\begin{align}
  b=2 \label{eq:b}
\end{align}

\begin{align}
  c=3 \label{eq:c}
\end{align}

\ref{eq:a}, \ref{eq:b}, \ref{eq:c}.
""")
        self.assertEqual(
            result,
            '<p><span id="equation-1" class="display-math env-align">'
            r'\begin{align}' '\n  a=1 ' r'\label{eq:a}' '\n'
            r'\tag*{(1)}\end{align}'
            '</span></p>\n'
            '<p><span id="equation-2" class="display-math env-align">'
            r'\begin{align}' '\n  b=2 ' r'\label{eq:b}' '\n'
            r'\tag*{(2)}\end{align}'
            '</span></p>\n'
            '<p><span id="equation-3" class="display-math env-align">'
            r'\begin{align}' '\n  c=3 ' r'\label{eq:c}' '\n'
            r'\tag*{(3)}\end{align}'
            '</span></p>\n'
            '<p>'
            '<a href="#equation-1" class="href-ref ref-eq">Eq.&nbsp;(1)</a>'
            ', '
            '<a href="#equation-2" class="href-ref ref-eq">Eq.&nbsp;(2)</a>'
            ', '
            '<a href="#equation-3" class="href-ref ref-eq">Eq.&nbsp;(3)</a>'
            '.</p>'
        )

    def test_starred_equation_no_label_allowed(self):
        # align* is unnumbered, so \label inside it raises an error
        environ = mk_flm_environ()
        with self.assertRaises(Exception):
            render_doc(environ, r"""
\begin{align*}
  x=1 \label{eq:star}
\end{align*}
""")


if __name__ == '__main__':
    unittest.main()
