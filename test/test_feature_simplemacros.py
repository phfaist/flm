import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer

from flm.feature import simplemacros as feature_simplemacros


def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class TestFeatureSimpleMacros(unittest.TestCase):

    maxDiff = None

    def test_simple(self):
        
        s = r'Test \mymacro.'
        
        environ = mk_flm_environ(macros_definitions={
            'macros': {
                'mymacro': {
                    'flm_text_replacement_textmode': '[my macro content]',
                }
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r"Test [my macro content]."
        )


    def test_simple_wflm(self):
        
        s = r'Test \textbf{macro content: \emph{a \mymacro\ b}}.'
        
        environ = mk_flm_environ(macros_definitions={
            'macros': {
                'mymacro': {
                    'flm_text_replacement_textmode': r'[my \textit{macro} content]',
                }
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test <span class="textbf">macro content: <span class="textit">a [my <span class="textit">macro</span> content] b</span></span>.'''
        )

    def test_with_deps(self):
        
        s = r'Test: \mymacro'
        
        environ = mk_flm_environ(macros_definitions={
            'macros': {
                'mymacro': {
                    'flm_text_replacement_textmode': r'Hey, \othermacro!',
                },
                'othermacro': {
                    'flm_text_replacement_textmode': r'\emph{YOU}',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: Hey, <span class="textit">YOU</span>!'''
        )


    def test_with_arguments_0(self):
        
        s = r'Test: \mymacro[Howdy]{Albert}'
        
        environ = mk_flm_environ(macros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {
                            'parser': '[',
                            #'argname': 'greeting',
                        },
                        {
                            'parser': '{',
                            #'argname': 'name',
                        },
                    ],
                    'flm_text_replacement_textmode': r'#1, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: Howdy, <span class="textit">Albert</span>!'''
        )

    def test_with_arguments_1(self):
        
        s = r'Test: \mymacro[Howdy]{Albert}'
        
        environ = mk_flm_environ(macros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {
                            'parser': '[',
                            'argname': 'greeting',
                        },
                        {
                            'parser': '{',
                            'argname': 'name',
                        },
                    ],
                    'flm_text_replacement_textmode': r'#{greeting}, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: Howdy, <span class="textit">Albert</span>!'''
        )


    def test_with_arguments_2(self):
        
        s = r'Test: \mymacro{Albert}'
        
        environ = mk_flm_environ(macros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {
                            'parser': '[',
                            'argname': 'greeting',
                        },
                        {
                            'parser': '{',
                            'argname': 'name',
                        },
                    ],
                    'default_argument_values': {
                        'greeting': 'Salut',
                    },
                    'flm_text_replacement_textmode': r'#{greeting}, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: Salut, <span class="textit">Albert</span>!'''
        )




if __name__ == '__main__':
    unittest.main()
