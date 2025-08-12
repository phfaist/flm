import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer

from flm.feature import annotations as feature_annotations


def mk_flm_environ(annotation_feature):
    features = standard_features()
    features.append(annotation_feature)
    return make_standard_environment(features)



class TestFeatureAnnotations(unittest.TestCase):

    maxDiff = None

    def test_simple_1(self):
        environ = mk_flm_environ( feature_annotations.FeatureAnnotations(
            macrodefs=[ ('phf', { 'initials': 'PhF' }), ('abc', {'initials': 'A.B.C.'}) ],
        ) )

        frag1 = environ.make_fragment(r"""
\phf{Test highlighted text}
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<span class="annotation annotation-highlight annotation-0"><span class="annotation-initials">PhF</span>Test highlighted text</span>
""".strip() .replace('\n', '')
        )

    def test_simple_2(self):
        environ = mk_flm_environ( feature_annotations.FeatureAnnotations(
            macrodefs=[ ('phf', { 'initials': 'PhF' }), ('abc', {'initials': 'A.B.C.'}) ],
        ) )

        frag1 = environ.make_fragment(r"""
\phf Test highlighted text\endphf
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<span class="annotation annotation-highlight annotation-0"><span class="annotation-initials">PhF</span>Test highlighted text</span>
""".strip() .replace('\n', '')
        )

    def test_simple_3(self):
        environ = mk_flm_environ( feature_annotations.FeatureAnnotations(
            macrodefs=[ ('phf', { 'initials': 'PhF' }), ('abc', {'initials': 'A.B.C.'}) ],
        ) )

        frag1 = environ.make_fragment(r"""
\phf[Test comment here]
        """ .strip())

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<span class="annotation annotation-comment annotation-0"><span class="annotation-initials">PhF</span>Test comment here</span>
""".strip() .replace('\n', '')
        )


    def test_simple_block(self):
        environ = mk_flm_environ( feature_annotations.FeatureAnnotations(
            macrodefs=[ ('phf', { 'initials': 'PhF' }), ('abc', {'initials': 'A.B.C.'}) ],
        ) )

        frag1 = environ.make_fragment(r"""
\phf
Test highlight
\begin{itemize}
\item one
\end{itemize}
\endphf
        """ .strip(), is_block_level=True)

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result .replace('\n', ''),
            r"""
<div class="annotation annotation-highlight annotation-0"><span class="annotation-initials">PhF</span><p>Test highlight</p><dl class="enumeration itemize"><dt>•</dt><dd><p>one</p></dd></dl></div>
""".strip() .replace('\n', '')
        )

    def test_simple_block_2(self):
        environ = mk_flm_environ( feature_annotations.FeatureAnnotations(
            macrodefs=[ ('phf', { 'initials': 'PhF' }), ('abc', {'initials': 'A.B.C.'}) ],
        ) )

        frag1 = environ.make_fragment(r"""
\abc[Test comment here]
        """ .strip(), is_block_level=True)

        doc = environ.make_document(frag1.render)

        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        print(result)
        self.assertEqual(
            result,
            r"""
<p><span class="annotation annotation-comment annotation-1"><span class="annotation-initials">A.B.C.</span>Test comment here</span></p>
""".strip() .replace('\n', '')
        )




if __name__ == '__main__':
    import logging
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
