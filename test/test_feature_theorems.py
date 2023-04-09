import unittest


from llm.llmenvironment import make_standard_environment
from llm.stdfeatures import standard_features
from llm.fragmentrenderer.html import HtmlFragmentRenderer

from llm.feature import theorems as feature_theorems


def mk_llm_environ_wthms(**kwargs):
    features = standard_features()
    features.append( feature_theorems.FeatureTheorems(**kwargs) )
    return make_standard_environment(features)



class TestFeatureTheorems(unittest.TestCase):

    maxDiff = None

    def test_simple_1(self):

        environ = mk_llm_environ_wthms()

        frag1 = environ.make_fragment(r"""
\begin{theorem}The square root of two is irrational.\end{theorem}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div class="theoremlike theorem"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;1</span> The square root of two is irrational.</p></div>
""".strip() .replace('\n', '')
        )

    def test_simple_1b(self):

        environ = mk_llm_environ_wthms()

        frag1 = environ.make_fragment(r"""
\begin{theorem}
The square root of two is irrational.
\end{theorem}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div class="theoremlike theorem"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;1</span> The square root of two is irrational.</p></div>
""".strip() .replace('\n', '')
        )


    def test_simple_2(self):

        environ = mk_llm_environ_wthms()

        frag1 = environ.make_fragment(r"""
\begin{theorem}[Irrationality of the square root of two]
The square root of two is irrational.
\end{theorem}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div class="theoremlike theorem"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;1 (Irrationality of the square root of two)</span> The square root of two is irrational.</p></div>
""".strip()
        )


    def test_shared_counter_simple(self):

        environ = mk_llm_environ_wthms()

        frag1 = environ.make_fragment(r"""
\begin{theorem}
The square root of two is irrational.
\end{theorem}

\begin{lemma}
The square root of three is irrational.
\end{lemma}

\begin{proposition}
The square root of four is rational.
\end{proposition}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div class="theoremlike theorem"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;1</span> The square root of two is irrational.</p></div>
<div class="theoremlike lemma"><p><span id="lemma-2" class="heading-level-theorem heading-inline">Lemma&nbsp;2</span> The square root of three is irrational.</p></div>
<div class="theoremlike proposition"><p><span id="proposition-3" class="heading-level-theorem heading-inline">Proposition&nbsp;3</span> The square root of four is rational.</p></div>
""".strip()
        )


    def test_separate_counters_simple(self):

        environ = mk_llm_environ_wthms(
            theorem_types={
                'theoremlike': {
                    'shared_numbering': False,
                    'counter_formatter': 'Roman',
                },
                'lemmaandproplike': {
                    'shared_numbering': True,
                    #'counter_formatter': 'alph',
                },
            },
            environments={
                'theoremlike': {
                    'theorem': {
                        'title': 'theorem',
                    },
                },
                'lemmaandproplike': {
                    'proposition': {
                        'title': 'proposition',
                    },
                    'lemma': {
                        'title': 'lemma',
                    },
                },
            },
            shared_counter_formatter='alph',
        )

        frag1 = environ.make_fragment(r"""
\begin{theorem}
The square root of two is irrational.
\end{theorem}

\begin{lemma}
The square root of three is irrational.
\end{lemma}

\begin{proposition}
The square root of four is rational.
\end{proposition}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div class="theoremlike theorem"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;I</span> The square root of two is irrational.</p></div>
<div class="lemmaandproplike lemma"><p><span id="lemma-1" class="heading-level-theorem heading-inline">Lemma&nbsp;a</span> The square root of three is irrational.</p></div>
<div class="lemmaandproplike proposition"><p><span id="proposition-2" class="heading-level-theorem heading-inline">Proposition&nbsp;b</span> The square root of four is rational.</p></div>
""".strip()
        )




    def test_ref_multi(self):

        environ = mk_llm_environ_wthms()

        frag1 = environ.make_fragment(r"""
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

Ref: \ref{thm:sqrt2,thm:sqrt4,thm:sqrt3}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<div class="theoremlike theorem"><p><span id="theorem-1" class="heading-level-theorem heading-inline">Theorem&nbsp;1</span> The square root of two is irrational.</p></div>
<div class="theoremlike theorem"><p><span id="theorem-2" class="heading-level-theorem heading-inline">Theorem&nbsp;2</span> The square root of three is irrational.</p></div>
<div class="theoremlike proposition"><p><span id="proposition-3" class="heading-level-theorem heading-inline">Proposition&nbsp;3</span> The square root of four is rational.</p></div>
<p>Ref: <a href="#theorem-1" class="href-ref refcnt-theorem">Theorems&nbsp;</a><a href="#theorem-1" class="href-ref refcnt-theorem">1</a> and&nbsp;<a href="#theorem-2" class="href-ref refcnt-theorem">2</a>, <a href="#proposition-3" class="href-ref refcnt-proposition">Proposition&nbsp;3</a></p>
""".strip()
        )






if __name__ == '__main__':
    unittest.main()
