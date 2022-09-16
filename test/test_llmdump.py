import os.path
import unittest
import json

from llm import llmdump
from llm.llmstd import LLMStandardEnvironment
from llm import llmstd

# ------------------

import pylatexenc.latexnodes.nodes as latexnodes_nodes

from llm.llmspecinfo import LLMMacroSpecBase
from llm.feature import Feature


class _AlwaysEqual:
    def __init__(self, value=None):
        self.value = value

    def __eq__(self, other):
        return True


class TestLLMDataDumper(unittest.TestCase):

    maxDiff = None

    def test_dump_node(self):

        environment = LLMStandardEnvironment()
        fragment = environment.make_fragment(r'\%')

        obj = fragment.nodes[0]
        assert obj.isNodeType(latexnodes_nodes.LatexMacroNode)

        dumper = llmdump.LLMDataDumper(environment=environment)
        dumper.add_dump('my_node', obj)

        dumped_data = dumper.get_data()

        self.assertEqual(
            dumped_data['objects'],
            {
                'my_node': {
                    '$type': 'LatexMacroNode',
                    'latex_walker': {'$reskey': _AlwaysEqual(),
                                     '$restype': 'LLMLatexWalker'},
                    'llm_is_block_heading': False,
                    'llm_is_block_level': False,
                    'llm_is_paragraph_break_marker': False,
                    'llm_specinfo': {'$skip': True},
                    'spec': {'$skip': True},
                    'macro_post_space': '',
                    'macroname': '%',
                    'nodeargd': {'$type': 'ParsedArguments',
                                 'argnlist': [],
                                 'arguments_spec_list': []},
                    'parsing_state': {'$reskey': _AlwaysEqual(),
                                      '$restype': 'LLMParsingState'},
                    'pos': 0,
                    'pos_end': 2
                }
            }
        )
        lw_reskey = dumped_data['objects']['my_node']['latex_walker']['$reskey']
        ps_reskey = dumped_data['objects']['my_node']['parsing_state']['$reskey']
        self.assertEqual(
            dumper.get_data()['resources'],
            {
                'LLMLatexWalker': {
                    lw_reskey: {
                        '$type': 'LLMLatexWalker',
                        's': r'\%',
                    },
                },
                'LLMParsingState': {
                    ps_reskey: {
                        '$type': 'LLMParsingState',
                        'comment_char': '%',
                        'enable_comments': None,
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
                        's': None
                    }
                }
            }
        )


    def test_dump_fragment(self):

        environment = LLMStandardEnvironment()
        fragment = environment.make_fragment(r'''Hello, \emph{world}!

\begin{enumerate}
\item Hi again.
\item And hello again.
\end{enumerate}
''')

        dumper = llmdump.LLMDataDumper(environment=environment)
        dumper.add_dump('my_fragment', fragment)

        dumped_data = dumper.get_data()

        # # JSON test data was generated in this way
        # print(json.dumps(dumped_data, indent=4))
        # with open(os.path.join(os.path.dirname(__file__), 'test_llmdump_data.json'),
        #           'w') as fw:
        #     json.dump(dumped_data, fw)

        with open(os.path.join(os.path.dirname(__file__), 'test_llmdump_data.json')) as f:

            # worry about different resource-keys ...  replace all $reskey's in
            # loaded_data by _AlwaysEqual()
            def object_hook(x):
                if '$reskey' in x:
                    x['$reskey'] = _AlwaysEqual(x['$reskey'])
                return x

            loaded_data = json.load(f, object_hook=object_hook)

        self.assertEqual(dumped_data['objects'], loaded_data['objects'])

        # pick out a parsing state to check it.  This picks out the '\item'
        # node, test with:
        #
        # cat test/test_llmdump_data.json | jq  \
        # '.objects.my_fragment.nodes.nodelist[4].nodelist.nodelist[1]'
        #
        dumped_item_macro = dumped_data['objects']['my_fragment']['nodes']['nodelist'][4] \
            ['nodelist']['nodelist'][1]
        dumped_reskey_ps = dumped_item_macro['parsing_state']['$reskey']

        loaded_item_macro = loaded_data['objects']['my_fragment']['nodes']['nodelist'][4] \
            ['nodelist']['nodelist'][1]
        loaded_reskey_ps = loaded_item_macro['parsing_state']['$reskey'].value

        self.assertEqual(dumped_data['resources']['LLMParsingState'][dumped_reskey_ps],
                         loaded_data['resources']['LLMParsingState'][loaded_reskey_ps])

        dumped_reskey_lw = dumped_item_macro['latex_walker']['$reskey']
        loaded_reskey_lw = loaded_item_macro['latex_walker']['$reskey'].value

        self.assertEqual(dumped_data['resources']['LLMLatexWalker'][dumped_reskey_lw],
                         loaded_data['resources']['LLMLatexWalker'][loaded_reskey_lw])
        


    def test_load_after_dump_fragment(self):

        environment = LLMStandardEnvironment()
        fragment = environment.make_fragment(r'''Hello, \emph{world}!

\begin{enumerate}
\item Hi again.
\item And hello again.
\end{enumerate}
''')

        dumper = llmdump.LLMDataDumper(environment=environment)
        dumper.add_dump('my_fragment', fragment)

        # even test via JSON
        dumped_data_json = json.dumps( dumper.get_data() )

        # reload data
        loader = llmdump.LLMDataLoader(json.loads(dumped_data_json),
                                       environment=environment)
        new_fragment = loader.get_object('my_fragment')

        self.assertEqual(fragment.llm_text, new_fragment.llm_text)

        self.assertEqual(
            fragment.nodes.nodelist[4].nodelist.nodelist[1].macroname, # \item
            new_fragment.nodes.nodelist[4].nodelist.nodelist[1].macroname, # \item
        )




if __name__ == '__main__':
    unittest.main()
