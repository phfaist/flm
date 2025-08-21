import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer



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




if __name__ == '__main__':
    unittest.main()
