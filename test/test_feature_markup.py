import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer

from flm.feature.markup import FeatureMarkup


def mk_flm_environ(markup_feature):
    features = standard_features()
    features.append(markup_feature)
    return make_standard_environment(features)



class TestFeatureMarkup(unittest.TestCase):

    maxDiff = None

    def test_simple_macro(self):
        
        s = r'Test \mymacro{my macro}.'

        environ = mk_flm_environ(FeatureMarkup(text_macros={
            'mymacro': {
                'text_formats': ['textbf']
            }
        }))
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            "Test <span class=\"textbf\">my macro</span>."
        )

    def test_simple_env(self):
        
        s = r'''
Test \begin{myenviron}
environment content
\end{myenviron}
'''

        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'myenviron': {
                'role': 'my-environ-role'
            }
        }))
        frag1 = environ.make_fragment(s, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            "<p>Test</p>\n<div class=\"my-environ-role\"> environment content </div>"
        )
        

    def test_simple_quot(self):
        
        s = r'''
Test \begin{quotation}
environment content
\end{quotation}
'''

        environ = mk_flm_environ(FeatureMarkup(semantic_environments={
            'quotation': {
                'role': 'quotation'
            }
        }))
        frag1 = environ.make_fragment(s, standalone_mode=True)
        
        latex_renderer = LatexFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(latex_renderer),
            r"""
Test

\begin{quotation} environment content \end{quotation}%
""".lstrip()
        )
        



if __name__ == '__main__':
    unittest.main()
