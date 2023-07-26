### BEGIN_TEST_FLM_SKIP
# no filesystem support in Transcrypt apparently
import os.path
### END_TEST_FLM_SKIP
import unittest
import json

from flm import flmdump
from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer import text as fragmentrenderer_text

# ------------------

import pylatexenc.latexnodes.nodes as latexnodes_nodes




def mk_flm_environ(**kwargs):
    features = standard_features(**kwargs)
    return make_standard_environment(features)





class _AlwaysEqual:
    def __init__(self, value=None):
        self.value = value

    def __eq__(self, other):
        return True


class TestFLMDataDumper(unittest.TestCase):

    maxDiff = None

    def test_dump_node(self):

        environment = mk_flm_environ()
        fragment = environment.make_fragment(r'\%')

        obj = fragment.nodes[0]
        assert obj.isNodeType(latexnodes_nodes.LatexMacroNode)

        dumper = flmdump.FLMDataDumper(environment=environment)
        dumper.add_object_dump('my_node', obj)

        dumped_data = dumper.get_data()

        self.assertEqual(
            dumped_data['dumps'],
            {
                'my_node': {
                    '$type': 'LatexMacroNode',
                    'latex_walker': {'$reskey': _AlwaysEqual(),
                                     '$restype': 'FLMLatexWalker'},
                    'flm_is_block_heading': False,
                    'flm_is_block_level': False,
                    'flm_is_paragraph_break_marker': False,
                    'flm_specinfo': {'$reskey': _AlwaysEqual(),
                                     '$restype': 'FLMSpecInfo'},
                    'spec': {'$reskey': _AlwaysEqual(),
                             '$restype': 'FLMSpecInfo'},
                    'macro_post_space': '',
                    'macroname': '%',
                    'nodeargd': {'$type': 'ParsedArguments',
                                 'argnlist': [],
                                 'arguments_spec_list': []},
                    'parsing_state': {'$reskey': _AlwaysEqual(),
                                      '$restype': 'FLMParsingState'},
                    'pos': 0,
                    'pos_end': 2
                }
            }
        )
        lw_reskey = dumped_data['dumps']['my_node']['latex_walker']['$reskey']
        ps_reskey = dumped_data['dumps']['my_node']['parsing_state']['$reskey']
        si_reskey = dumped_data['dumps']['my_node']['spec']['$reskey']
        self.assertEqual(si_reskey, dumped_data['dumps']['my_node']['flm_specinfo']['$reskey'])
        self.assertEqual(
            dumper.get_data()['resources'],
            {
                'FLMLatexWalker': {
                    lw_reskey: {
                        '$type': 'FLMLatexWalker',
                        's': r'\%',
                    },
                },
                'FLMParsingState': {
                    ps_reskey: {
                        '$type': 'FLMParsingState',
                        'comment_start': '%%',
                        'enable_comments': True,
                        'enable_double_newline_paragraphs': True,
                        'enable_environments': True,
                        'enable_groups': True,
                        'enable_macros': True,
                        'enable_math': True,
                        'enable_specials': True,
                        'forbidden_characters': '$%',
                        'in_math_mode': False,
                        'is_block_level': None,
                        'latex_context': {'$skip': True},
                        'latex_display_math_delimiters': [['\\[',
                                                           '\\]']],
                        'latex_group_delimiters': [['{', '}']],
                        'latex_inline_math_delimiters': [['\\(',
                                                          '\\)']],
                        'macro_alpha_chars': 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ',
                        'macro_escape_char': '\\',
                        'math_mode_delimiter': None,
                        's': None,
                    },
                },
                'FLMSpecInfo': {
                    si_reskey: {
                        '$type': 'flm.flmspecinfo:ConstantValueMacro',
                        'macroname': '%',
                        'value': '%',
                    },
                },
            }
        )


### BEGIN_TEST_FLM_SKIP
# no fs support in Transcrypt
    def test_dump_fragment(self):

        environment = mk_flm_environ()
        fragment = environment.make_fragment(r'''Hello, \emph{world}!

\begin{enumerate}
\item Hi again.
\item And hello again.
\end{enumerate}
''')

        dumper = flmdump.FLMDataDumper(environment=environment)
        dumper.add_object_dump('my_fragment', fragment)

        dumped_data = dumper.get_data()

        # JSON test data was generated in this way
        print(json.dumps(dumped_data, indent=4))
        with open(os.path.join(os.path.dirname(__file__), 'test_flmdump_data_NEW.json'),
                  'w') as fw:
            json.dump(dumped_data, fw, indent=4)

        with open(os.path.join(os.path.dirname(__file__), 'test_flmdump_data.json')) as f:

            # worry about different resource-keys ...  replace all $reskey's in
            # loaded_data by _AlwaysEqual()
            def object_hook(x):
                if '$reskey' in x:
                    x['$reskey'] = _AlwaysEqual(x['$reskey'])
                return x

            loaded_data = json.load(f, object_hook=object_hook)

        self.assertEqual(dumped_data['dumps'], loaded_data['dumps'])

        # pick out a parsing state to check it.  This picks out the '\item'
        # node, test with:
        #
        # cat test/test_flmdump_data.json | jq  \
        # '.dumps.my_fragment.nodes.nodelist[4]' # etc. .nodelist.nodelist[1]'
        #
        nref = dumped_data['dumps']['my_fragment']['nodes']['nodelist'][4]['$reskey']
        nref = dumped_data['resources']['LatexNode'][nref] \
            ['nodelist']['nodelist'][1]['$reskey']
        dumped_item_macro = dumped_data['resources']['LatexNode'][nref]
        dumped_reskey_ps = dumped_item_macro['parsing_state']['$reskey']

        nref = loaded_data['dumps']['my_fragment']['nodes']['nodelist'][4]['$reskey'].value
        nref = loaded_data['resources']['LatexNode'][nref] \
            ['nodelist']['nodelist'][1]['$reskey'].value
        loaded_item_macro = loaded_data['resources']['LatexNode'][nref]
        loaded_reskey_ps = loaded_item_macro['parsing_state']['$reskey'].value

        self.assertEqual(dumped_data['resources']['FLMParsingState'][dumped_reskey_ps],
                         loaded_data['resources']['FLMParsingState'][loaded_reskey_ps])

        dumped_reskey_lw = dumped_item_macro['latex_walker']['$reskey']
        loaded_reskey_lw = loaded_item_macro['latex_walker']['$reskey'].value

        self.assertEqual(dumped_data['resources']['FLMLatexWalker'][dumped_reskey_lw],
                         loaded_data['resources']['FLMLatexWalker'][loaded_reskey_lw])
        

### END_TEST_FLM_SKIP

    def test_load_after_dump_fragment(self):

        environment = mk_flm_environ()
        fragment = environment.make_fragment(r'''Hello, \emph{world}!

\begin{enumerate}
\item Hi again.
\item And hello again.
\end{enumerate}
''')

        dumper = flmdump.FLMDataDumper(environment=environment)
        dumper.add_object_dump('my_fragment', fragment)

        # even test via JSON
        dumped_data = dumper.get_data()
        dumped_data_json = json.dumps( dumped_data )


        # Help debug JSON output
        print(json.dumps(dumped_data, indent=4))
        # with open(os.path.join(os.path.dirname(__file__), '_test_dumped_data.json'),
        #           'w') as fw:
        #     json.dump(dumped_data, fw, indent=4)


        # reload data
        loader = flmdump.FLMDataLoader(json.loads(dumped_data_json),
                                       environment=environment)
        new_fragment = loader.get_object_dump('my_fragment')

        # print('re-loaded fragment: ', repr(new_fragment))
        # print('nodes: ', repr(new_fragment.nodes))

        self.assertEqual(fragment.flm_text, new_fragment.flm_text)

        self.assertEqual(
            fragment.nodes.nodelist[4].nodelist.nodelist[1].macroname, # \item
            new_fragment.nodes.nodelist[4].nodelist.nodelist[1].macroname, # \item
        )


    def test_render_after_dump_and_load_fragment(self):

        environment = mk_flm_environ()
        fragment = environment.make_fragment(
            r'''
Hello, \emph{world}!

\begin{enumerate}
\item Hi again.
\item And hello again.
\end{enumerate}
'''.lstrip(),
            standalone_mode=True,
        )

        dumper = flmdump.FLMDataDumper(environment=environment)
        dumper.add_object_dump('my_fragment', fragment)

        # even test via JSON
        dumped_data_json = json.dumps( dumper.get_data() )

        # reload data
        loader = flmdump.FLMDataLoader(json.loads(dumped_data_json),
                                       environment=environment)
        new_fragment = loader.get_object_dump('my_fragment')

        # print('re-loaded fragment = ', repr(new_fragment))

        self.assertEqual(fragment.flm_text, new_fragment.flm_text)
        
        # now, render the content to e.g. text

        text_renderer = fragmentrenderer_text.TextFragmentRenderer()

        result_1 = new_fragment.render_standalone(text_renderer)
        
        self.assertEqual(
            r"""
Hello, world!

  1. Hi again.

  2. And hello again.
""".strip(),
            result_1
        )



if __name__ == '__main__':
    unittest.main()
