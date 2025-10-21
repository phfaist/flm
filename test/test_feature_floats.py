import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)


class TestFeatureFloats(unittest.TestCase):

    maxDiff = None

    def test_simple_1(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"""
\begin{figure}
\includegraphics{fig/test}
\end{figure}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print('result:', result)
        self.assertEqual(
            result,
            r"""
<figure class="float float-figure"><div class="float-contents"><img src="fig/test"></div></figure>
""".strip() .replace('\n', '')
        )


    def test_simple_captiononly_1(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"""
\begin{figure}
\includegraphics{fig/test}
\caption{Here is my test figure}
\end{figure}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print('result:', result)
        self.assertEqual(
            result,
            r"""
<figure class="float float-figure"><div class="float-contents"><img src="fig/test"></div>
<figcaption class="float-caption-content"><span><span class="float-no-number">Figure</span>: Here is my test figure</span></figcaption></figure>
""".strip()
        )


    def test_simple_numberonly_1(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"""
\begin{figure}
\includegraphics{fig/test}
\label{figure:numbered}
\end{figure}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print('result:', result)
        self.assertEqual(
            result,
            r"""
<figure id="figure-1" class="float float-figure"><div class="float-contents"><img src="fig/test"></div>
<figcaption class="float-caption-content"><span><span class="float-number">Figure&nbsp;1</span></span></figcaption></figure>
""".strip()
        )


    def test_simple_numbercaption_1(self):

        environ = mk_flm_environ()

        frag1 = environ.make_fragment(r"""
\begin{figure}
\includegraphics{fig/test}
\label{figure:numbered}
\caption{My test figure}
\end{figure}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        fr.float_caption_title_separator = '. '
        result, _ = doc.render(fr)
        print('result:', result)
        self.assertEqual(
            result,
            r"""
<figure id="figure-1" class="float float-figure"><div class="float-contents"><img src="fig/test"></div>
<figcaption class="float-caption-content"><span><span class="float-number">Figure&nbsp;1</span>. My test figure</span></figcaption></figure>
""".strip()
        )








if __name__ == '__main__':
    unittest.main()
