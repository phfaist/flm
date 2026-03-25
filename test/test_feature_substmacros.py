import unittest

from pylatexenc.latexnodes import LatexWalkerLocatedError

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.fragmentrenderer.text import TextFragmentRenderer
from flm.fragmentrenderer.latex import LatexFragmentRenderer
from flm.fragmentrenderer.markdown import MarkdownFragmentRenderer
from flm.flmrecomposer.purelatex import FLMPureLatexRecomposer
from flm.feature.substmacros import (
    FeatureSubstMacros,
    SubstitutionMacro,
    SubstitutionEnvironment,
    SubstitutionSpecials,
)



def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)



class TestFeatureSubstMacros(unittest.TestCase):

    maxDiff = None

    def test_simple(self):
        
        s = r'Test \mymacro.'
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'content': '[my macro content]',
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
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'content': r'[my \textit{macro} content]',
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
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'content': r'Hey, \othermacro!',
                },
                'othermacro': {
                    'content': r'\emph{YOU}',
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
        
        environ = mk_flm_environ(substmacros_definitions={
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
                    'content': r'#1, \emph{#2}!',
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
        
        environ = mk_flm_environ(substmacros_definitions={
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
                    'content': r'#{greeting}, \emph{#2}!',
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
        
        environ = mk_flm_environ(substmacros_definitions={
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
                    'content': r'\textbf{#{greeting}}, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: <span class="textbf">Salut</span>, <span class="textit">Albert</span>!'''
        )


    def test_with_arguments_3(self):
        
        s = r'Test: \mymacro[]{Albert}'
        
        environ = mk_flm_environ(substmacros_definitions={
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
                    'content': r'\textbf{#{greeting}}, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: <span class="textbf"></span>, <span class="textit">Albert</span>!'''
        )


    def test_with_arguments_4(self):
        
        s = r'Test: \mymacro[]{Albert}'
        
        environ = mk_flm_environ(substmacros_definitions={
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
                        1: 'Salut',
                    },
                    'content': r'\textbf{#1}, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: <span class="textbf"></span>, <span class="textit">Albert</span>!'''
        )



    def test_with_arguments_mathmode(self):
        
        s = r'Test: \(\calN(\rho)\)!'
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'calN': {
                    'content': r'\mathcal{N}',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: <span class="inline-math">\(\mathcal{N}(\rho)\)</span>!'''
        )



    def test_with_arguments_blck(self):
        
        s = r'''Block 1

\mymacro{Albert}

Block 3
'''
        
        environ = mk_flm_environ(substmacros_definitions={
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
                        'greeting': 'Hello',
                    },
                    'content': r'#1, \emph{#2}!',
                },
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=True, standalone_mode=True)

        print('ps: ', frag1.nodes.parsing_state, ' = ', frag1.nodes.parsing_state.get_fields())
        print('*** blocks[1] = ', frag1.nodes.flm_blocks[1])
        print('*** blocks[1].nodelist = ', frag1.nodes.flm_blocks[1].nodelist)

        self.assertEqual(len(frag1.nodes.flm_blocks), 3)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''<p>Block 1</p>
<p>Hello, <span class="textit">Albert</span>!</p>
<p>Block 3</p>'''
        )



    def test_specials(self):
        
        s = r'Test! Hello.'
        
        environ = mk_flm_environ(substmacros_definitions={
            'specials': {
                '!': {
                    'content': '[exclamation mark]',
                }
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r"Test[exclamation mark] Hello."
        )



    def test_specials_arg(self):
        
        s = r'Test!+ Hello.'
        
        environ = mk_flm_environ(substmacros_definitions={
            'specials': {
                '!': {
                    'content': '[exclamation mark, #{arg1}, #{arg2}]',
                    'arguments_spec_list': [
                        {
                            'parser': '[',
                            'argname': 'arg1'
                        },
                        {
                            'parser': '{',
                            'argname': 'arg2'
                        },
                    ],
                    'default_argument_values': {
                        1: '|-|',
                    },
                }
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r"Test[exclamation mark, |-|, +] Hello."
        )


    def test_env(self):
        
        s = r'''Test
\begin{myenv}
Hello.
\end{myenv}'''
        
        environ = mk_flm_environ(substmacros_definitions={
            'environments': {
                'myenv': {
                    'content': '[[[#{body}]]]',
                }
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test [[[ Hello. ]]]'''
        )


    def test_subst_url(self):
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'myeczoolink': {
                    'arguments_spec_list': [ '{' ],
                    'content': r'\url{https://errorcorrectionzoo.org/c/#1}'
                }
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\myeczoolink{surface}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)
        
        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''<a href="https://errorcorrectionzoo.org/c/surface" class="href-href">errorcorrectionzoo.org/c/surface</a>'''
        )


    def test_env_arg(self):
        
        s = r'''Test
\begin{myenv}{ccc}
Hello.
\end{myenv}'''
        
        environ = mk_flm_environ(substmacros_definitions={
            'environments': {
                'myenv': {
                    'content': '[#1][[[#{body}]]]',
                    'arguments_spec_list': '{',
                }
            }
        })
        frag1 = environ.make_fragment(s, is_block_level=False, standalone_mode=True)
        
        html_renderer = HtmlFragmentRenderer()

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test [ccc][[[ Hello. ]]]'''
        )



    def test_subst_IfNoValueTF(self):
        
        environ = mk_flm_environ(substmacros_definitions={
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
                        'greeting': 'Hello',
                    },
                    'content': r'\IfNoValueTF{#1}{Hello, \emph{#2}!}{#1, \emph{#2}.}',
                },
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\mymacro{Albert}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Hello, <span class="textit">Albert</span>!'''
        )

        s2 = r'''\mymacro[Howdy]{Albert}'''
        frag2 = environ.make_fragment(s2, standalone_mode=True)

        self.assertEqual(
            frag2.render_standalone(html_renderer),
            r'''Howdy, <span class="textit">Albert</span>.'''
        )

        s3 = r'''\mymacro[]{Albert}'''
        frag3 = environ.make_fragment(s3, standalone_mode=True)

        self.assertEqual(
            frag3.render_standalone(html_renderer),
            r''', <span class="textit">Albert</span>.'''
        )

        s4 = r'''\mymacro[{}]{Albert}'''
        frag4 = environ.make_fragment(s4, standalone_mode=True)

        self.assertEqual(
            frag4.render_standalone(html_renderer),
            r''', <span class="textit">Albert</span>.'''
        )

    def test_subst_IfNoValueT_IfNoValueF(self):
        
        environ = mk_flm_environ(substmacros_definitions={
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
                        'greeting': 'Hello',
                    },
                    'content': r'Test: \IfNoValueF{#{greeting}}{#1: }\IfNoValueT{#1}{Hello, }\emph{#2}!',
                },
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\mymacro{Albert}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: Hello, <span class="textit">Albert</span>!'''
        )

        s2 = r'''\mymacro[Howdy]{Albert}'''
        frag2 = environ.make_fragment(s2, standalone_mode=True)

        self.assertEqual(
            frag2.render_standalone(html_renderer),
            r'''Test: Howdy: <span class="textit">Albert</span>!'''
        )

    def test_subst_IfBoolean(self):
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {
                            'parser': '*',
                            'argname': 'star',
                        },
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
                        'greeting': 'Hello',
                    },
                    'content': r'Test: \IfBooleanTF{#{star}}{STAR}{NOSTAR} \IfBooleanT{#1}{T-STAR}-\IfBooleanF{#1}{F-NOSTAR}'
                },
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\mymacro*[Hi]{Albert}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: STAR T-STAR-'''
        )

        s2 = r'''\mymacro{Albert}'''
        frag2 = environ.make_fragment(s2, standalone_mode=True)

        self.assertEqual(
            frag2.render_standalone(html_renderer),
            r'''Test: NOSTAR -F-NOSTAR'''
        )


    def test_subst_IfValue(self):
        
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {
                            'parser': '*',
                            'argname': 'star',
                        },
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
                        'greeting': 'Hello',
                    },
                    'content': r'Test: \IfValueTF{#{greeting}}{G}{NOG} \IfValueT{#1}{T-G}-\IfValueF{#1}{F-NOG}'
                },
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\mymacro*[Hi]{Albert}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: G T-G-'''
        )

        s2 = r'''\mymacro{Albert}'''
        frag2 = environ.make_fragment(s2, standalone_mode=True)

        self.assertEqual(
            frag2.render_standalone(html_renderer),
            r'''Test: NOG -F-NOG'''
        )

        s3 = r'''\mymacro*[]{Albert}'''
        frag3 = environ.make_fragment(s3, standalone_mode=True)

        self.assertEqual(
            frag3.render_standalone(html_renderer),
            r'''Test: G T-G-'''
        )

        s4 = r'''\mymacro*[{}]{Albert}'''
        frag4 = environ.make_fragment(s4, standalone_mode=True)

        self.assertEqual(
            frag4.render_standalone(html_renderer),
            r'''Test: G T-G-'''
        )


    def test_subst_ifblank_notblank(self):
        
        environ = mk_flm_environ(substmacros_definitions={
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
                        'greeting': 'Hello',
                    },
                    'content': r'''
Test: \ifblank{#1}{1BLK}{1NOBLK:#1} \ifblank{#2}{2BLK}{2NOBLK:#2}
\notblank{#1}{1NOBLK:#1}{1BLK} \notblank{#2}{2NOBLK:#2}{2BLK}
'''.strip().replace('\n', ' ')
                },
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\mymacro[Hi]{Albert}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: 1NOBLK:Hi 2NOBLK:Albert 1NOBLK:Hi 2NOBLK:Albert'''
        )

        s2 = r'''\mymacro{Albert}'''
        frag2 = environ.make_fragment(s2, standalone_mode=True)

        self.assertEqual(
            frag2.render_standalone(html_renderer),
            r'''Test: 1BLK 2NOBLK:Albert 1BLK 2NOBLK:Albert'''
        )

        s3 = r'''\mymacro{}'''
        frag3 = environ.make_fragment(s3, standalone_mode=True)

        self.assertEqual(
            frag3.render_standalone(html_renderer),
            r'''Test: 1BLK 2BLK 1BLK 2BLK'''
        )

    def test_subst_notblank_nested(self):
        
        environ = mk_flm_environ(substmacros_definitions={
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
                        'greeting': 'Hello',
                    },
                    'content': r'''
Test: \notblank{#1}{\notblank{#2}{1NOBLK:#1,2NOBLK:#2}{1NOBLK:#1,2BLK}}{\notblank{#2}{1BLK,2NOBLK:#2}{1BLK,2BLK}}
'''.strip().replace('\n', ' ')
                },
            }
        })

        html_renderer = HtmlFragmentRenderer()

        s1 = r'''\mymacro[Hi]{Albert}'''
        frag1 = environ.make_fragment(s1, standalone_mode=True)

        self.assertEqual(
            frag1.render_standalone(html_renderer),
            r'''Test: 1NOBLK:Hi,2NOBLK:Albert'''
        )

        s2 = r'''\mymacro[]{Albert}'''
        frag2 = environ.make_fragment(s2, standalone_mode=True)

        self.assertEqual(
            frag2.render_standalone(html_renderer),
            r'''Test: 1BLK,2NOBLK:Albert'''
        )

        s3 = r'''\mymacro[]{Albert}'''
        frag3 = environ.make_fragment(s3, standalone_mode=True)

        self.assertEqual(
            frag3.render_standalone(html_renderer),
            r'''Test: 1BLK,2NOBLK:Albert'''
        )

        s4 = r'''\mymacro[Hi]{}'''
        frag4 = environ.make_fragment(s4, standalone_mode=True)

        self.assertEqual(
            frag4.render_standalone(html_renderer),
            r'''Test: 1NOBLK:Hi,2BLK'''
        )

        s5 = r'''\mymacro[]{}'''
        frag5 = environ.make_fragment(s5, standalone_mode=True)

        self.assertEqual(
            frag5.render_standalone(html_renderer),
            r'''Test: 1BLK,2BLK'''
        )




class TestFeatureSubstMacrosInit(unittest.TestCase):

    def test_init_none_definitions(self):
        f = FeatureSubstMacros(None)
        self.assertEqual(f.definitions, {'macros': {}, 'environments': {}, 'specials': {}})

    def test_init_partial_definitions(self):
        f = FeatureSubstMacros({'macros': {'m': {'content': 'x'}}})
        self.assertEqual(f.definitions['macros'], {'m': {'content': 'x'}})
        self.assertEqual(f.definitions['environments'], {})
        self.assertEqual(f.definitions['specials'], {})

    def test_feature_name(self):
        f = FeatureSubstMacros(None)
        self.assertEqual(f.feature_name, 'macros')
        self.assertEqual(f.feature_title, 'Custom macros definitions')

    def test_add_latex_context_definitions(self):
        f = FeatureSubstMacros({
            'macros': {'m': {'content': 'x'}},
            'environments': {'e': {'content': '#{body}'}},
        })
        defs = f.add_latex_context_definitions()
        self.assertEqual(len(defs['macros']), 1)
        self.assertEqual(defs['macros'][0].macroname, 'm')
        self.assertEqual(len(defs['environments']), 1)
        self.assertEqual(defs['environments'][0].environmentname, 'e')
        self.assertEqual(len(defs['specials']), 0)


class TestSubstitutionCallableGetWhat(unittest.TestCase):

    def test_get_what_macro(self):
        m = SubstitutionMacro(macroname='test', content='x')
        self.assertEqual(m.get_what(), '\u201c\\test\u201d')

    def test_get_what_environment(self):
        e = SubstitutionEnvironment(environmentname='myenv', content='x')
        self.assertEqual(e.get_what(), '\u201c\\begin{myenv}...\\end{..}\u201d')

    def test_get_what_specials(self):
        s = SubstitutionSpecials(specials_chars='@', content='x')
        self.assertEqual(s.get_what(), '\u201c@\u201d')


class TestSubstMacrosRenderers(unittest.TestCase):

    maxDiff = None

    def test_simple_text_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'content': '[my macro content]',
                }
            }
        })
        frag = environ.make_fragment(r'Test \mymacro.', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Test [my macro content].'
        )

    def test_simple_latex_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'content': '[my macro content]',
                }
            }
        })
        frag = environ.make_fragment(r'Test \mymacro.', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(LatexFragmentRenderer()),
            'Test [my macro content].'
        )

    def test_simple_markdown_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'content': '[my macro content]',
                }
            }
        })
        frag = environ.make_fragment(r'Test \mymacro.', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(MarkdownFragmentRenderer()),
            r'Test \[my macro content\]\.'
        )

    def test_args_text_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Salut'},
                    'content': r'\textbf{#{greeting}}, \emph{#2}',
                }
            }
        })
        frag = environ.make_fragment(r'\mymacro{Albert}', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Salut, Albert'
        )

    def test_args_latex_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Salut'},
                    'content': r'\textbf{#{greeting}}, \emph{#2}',
                }
            }
        })
        frag = environ.make_fragment(r'\mymacro{Albert}', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(LatexFragmentRenderer()),
            r'\textbf{Salut}, \textit{Albert}'
        )

    def test_args_markdown_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Salut'},
                    'content': r'\textbf{#{greeting}}, \emph{#2}',
                }
            }
        })
        frag = environ.make_fragment(r'\mymacro{Albert}', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(MarkdownFragmentRenderer()),
            '**Salut**, *Albert*'
        )

    def test_specials_text_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'specials': {
                '@': {
                    'content': '[at-sign]',
                }
            }
        })
        frag = environ.make_fragment(r'user@domain', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'user[at-sign]domain'
        )

    def test_env_text_renderer(self):
        environ = mk_flm_environ(substmacros_definitions={
            'environments': {
                'myenv': {
                    'content': r'[[[#{body}]]]',
                }
            }
        })
        frag = environ.make_fragment(
            r'Test \begin{myenv}Hello.\end{myenv}',
            standalone_mode=True,
        )
        self.assertEqual(
            frag.render_standalone(TextFragmentRenderer()),
            'Test [[[Hello.]]]'
        )


class TestSubstMacrosContentDict(unittest.TestCase):

    maxDiff = None

    def test_content_dict_textmode(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'calN': {
                    'content': {'textmode': 'CALLIGRAPHIC-N', 'mathmode': r'\mathcal{N}'},
                },
            }
        })
        frag = environ.make_fragment(r'Test \calN here', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Test CALLIGRAPHIC-Nhere'
        )

    def test_content_dict_mathmode(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'calN': {
                    'content': {'textmode': 'CALLIGRAPHIC-N', 'mathmode': r'\mathcal{N}'},
                },
            }
        })
        frag = environ.make_fragment(r'Test \(\calN(\rho)\)', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            r'Test <span class="inline-math">\(\mathcal{N}(\rho)\)</span>'
        )

    def test_content_textmode_only_raises_in_mathmode(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'calN': {
                    'content': {'textmode': 'N-textmode'},
                },
            }
        })
        with self.assertRaises(LatexWalkerLocatedError):
            environ.make_fragment(r'Test \(\calN\) here', standalone_mode=True)

    def test_empty_content(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {},
            }
        })
        frag = environ.make_fragment(r'Test \mymacro here', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Test here'
        )

    def test_string_argspec_shorthand(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': ['{'],
                    'content': r'Arg is #1',
                }
            }
        })
        frag = environ.make_fragment(r'\mymacro{hello}', standalone_mode=True)
        self.assertEqual(
            frag.render_standalone(HtmlFragmentRenderer()),
            'Arg is hello'
        )


class TestSubstMacrosErrors(unittest.TestCase):

    def test_invalid_argument_number(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': ['{'],
                    'content': r'Arg is #3',
                }
            }
        })
        with self.assertRaises(LatexWalkerLocatedError):
            environ.make_fragment(r'\mymacro{hello}', standalone_mode=True)


class TestSubstMacrosRecomposer(unittest.TestCase):

    maxDiff = None

    def test_recompose_macro_with_args(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Hello'},
                    'content': r'\textbf{#{greeting}}, \emph{#2}',
                },
            }
        })
        frag = environ.make_fragment(r'\mymacro[Hi]{Albert}', standalone_mode=True)
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'\textbf{Hi}, \emph{Albert}')

    def test_recompose_macro_default_arg(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Hello'},
                    'content': r'\textbf{#{greeting}}, \emph{#2}',
                },
            }
        })
        frag = environ.make_fragment(r'\mymacro{Albert}', standalone_mode=True)
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'\textbf{Hello}, \emph{Albert}')

    def test_recompose_math_mode(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'calN': {
                    'content': r'\mathcal{N}',
                },
            }
        })
        frag = environ.make_fragment(r'Test \(\calN(\rho)\)', standalone_mode=True)
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'Test \(\mathcal{N}(\rho)\)')

    def test_recompose_environment(self):
        environ = mk_flm_environ(substmacros_definitions={
            'environments': {
                'myenv': {
                    'content': r'[[[#{body}]]]',
                }
            }
        })
        frag = environ.make_fragment(
            r'Test \begin{myenv}Hello.\end{myenv}',
            standalone_mode=True,
        )
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], 'Test [[[Hello.]]]')

    def test_recompose_specials(self):
        environ = mk_flm_environ(substmacros_definitions={
            'specials': {
                '@': {
                    'content': '[at]',
                }
            }
        })
        frag = environ.make_fragment(r'user@domain', standalone_mode=True)
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], 'user[at]domain')

    def test_recompose_ifnovaluetf_not_provided(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Hello'},
                    'content': r'\IfNoValueTF{#1}{Hello, \emph{#2}}{#1, \emph{#2}}',
                },
            }
        })
        frag = environ.make_fragment(r'\mymacro{Albert}', standalone_mode=True)
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'Hello, \emph{Albert}')

    def test_recompose_ifnovaluetf_provided(self):
        environ = mk_flm_environ(substmacros_definitions={
            'macros': {
                'mymacro': {
                    'arguments_spec_list': [
                        {'parser': '[', 'argname': 'greeting'},
                        {'parser': '{', 'argname': 'name'},
                    ],
                    'default_argument_values': {'greeting': 'Hello'},
                    'content': r'\IfNoValueTF{#1}{Hello, \emph{#2}}{#1, \emph{#2}}',
                },
            }
        })
        frag = environ.make_fragment(r'\mymacro[Howdy]{Albert}', standalone_mode=True)
        r = FLMPureLatexRecomposer({})
        res = r.recompose_pure_latex(frag.nodes)
        self.assertEqual(res['latex'], r'Howdy, \emph{Albert}')


if __name__ == '__main__':
    unittest.main()
