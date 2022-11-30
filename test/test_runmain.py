import unittest

### BEGIN_TEST_LLM_SKIP

import io

from llm.runmain import runmain, LLMMainArguments, recursive_assign_defaults


class TestRunMain(unittest.TestCase):
    
    maxDiff = None

    def test_simple_text(self):
        sout = io.StringIO()
        runmain(LLMMainArguments(
            output=sout,
            llm_content=r"Hello \emph{world}!  Looking great today.",
            format='text'
        ))
        self.assertEqual(sout.getvalue(), "Hello world! Looking great today.\n")

    def test_simple_html(self):
        sout = io.StringIO()
        runmain(LLMMainArguments(
            output=sout,
            llm_content=r"Hello \emph{world}!  Looking great today.",
            format='html'
        ))
        self.assertEqual(
            sout.getvalue(),
            """Hello <span class="textit">world</span>! Looking great today.\n"""
        )


    def test_simple_frontmatter(self):
        sout = io.StringIO()
        runmain(LLMMainArguments(
            output=sout,
            llm_content=r"""---
llm:
  parsing:
    comment_start: '##'
    enable_comments: true
    dollar_inline_math_mode: true
    force_block_level: true
---
Hello \emph{world}!  Let $x$ and $y$ be real numbers. ## comments configured like this!
""",
            format='html',
            suppress_final_newline=True,
        ))
        self.assertEqual(
            sout.getvalue(),
            """<p>Hello <span class="textit">world</span>! Let <span class="inline-math">$x$</span> and <span class="inline-math">$y$</span> be real numbers.</p>"""
        )






class TestRecursiveAssignDefaults(unittest.TestCase):

    maxDiff = None

    def test_simple(self):

        d = recursive_assign_defaults([
            {
                'key1': 'goodvalue1',
            },
            {
                'key1': 'value1',
                'key2': 'value2',
            },
            {
                'key3': 'value3',
            },
        ])

        self.assertEqual(d, {
            'key1': 'goodvalue1',
            'key2': 'value2',
            'key3': 'value3'
        })

    def test_simple_recursive(self):

        d = recursive_assign_defaults([
            {
                'key1': 'goodvalue1',
                'prop1': {
                    'a': 'AAA',
                    'b': 'BBB',
                },
                'prop2': {
                    'x': 'XXX',
                    'y': 'YYY',
                },
            },
            {
                'key1': 'value1',
                'prop2': {
                    'z': 'ZZZ',
                    'w': { 'one': 'W', 'two': 'WW' },
                },
            },
            {
                'key3': 'value3',
                'prop1': {
                    'b': "BBB-CCC",
                },
                'prop2': {
                    'w': {
                        'two': { 'NN': 'WWNN' },
                        'three': 'WWW'
                    },
                },
            },
        ])

        self.assertEqual(d, {
            'key1': 'goodvalue1',
            'key3': 'value3',
            'prop1': {
                'a': 'AAA',
                'b': 'BBB',
            },
            'prop2': {
                'x': 'XXX',
                'y': 'YYY',
                'z': 'ZZZ',
                'w': {
                    'one': 'W',
                    'two': 'WW',
                    'three': 'WWW',
                },
            },
        })


    # -- test presets --

    def test_preset_defaults(self):

        d = recursive_assign_defaults([
            {
                'k1': [
                    { '$defaults': None,},
                ],
                'prop': {
                    'x': [
                        'X',
                        'Y',
                        {'$defaults': True },
                        'Z'
                    ]
                },
            },
            {
            },
            {
                'prop': {
                    'z': True
                },
            },
            {
                'k1': [
                    'one',
                    'two',
                    {'$defaults': True},
                ],
                'prop': {
                    'x': [ {'alpha': 1}, 'beta' ]
                },
            },
            {
                'k1': [
                    {'more': True},
                    {'$defaults': True}, # last $defaults expands to nothing
                ]
            },
        ])

        self.assertEqual(d, {
            'k1': [ 'one', 'two', {'more': True} ],
            'prop': {
                'x': [
                    'X', 'Y',
                    {'alpha': 1}, 'beta',
                    'Z'
                ],
                'z': True
            },
        })

    def test_preset_remove_item(self):
        
        d = recursive_assign_defaults([
            {
                'prop': [
                    {'$defaults': True },
                    {'$remove-item': 'A'},
                ],
            },
            {
                'prop': [
                    {'name': 'A',
                     'config': { 'hello': True }},
                    {'name': 'B',
                     'config': { 'hello': False }},
                ],
            },
        ])

        self.assertEqual(d, {
            'prop': [
                {'name': 'B',
                 'config': { 'hello': False }}
            ]
        })

    def test_preset_merge_config(self):
        
        d = recursive_assign_defaults([
            {
                'prop': [
                    {'$defaults': True},
                    {'$merge-config': { 'name': 'A',
                                        'config': { 'hello2': 'two' } } },
                ],
            },
            {
                'prop': [
                    {'name': 'A',
                     'config': { 'hello': True }},
                    {'name': 'B',
                     'config': { 'hello': False }},
                ],
            },
        ])

        self.assertEqual(d, {
            'prop': [
                {'name': 'A',
                 'config': { 'hello': True, 'hello2': 'two' }},
                {'name': 'B',
                 'config': { 'hello': False }},
            ]
        })


if __name__ == '__main__':
    unittest.main()

### END_TEST_LLM_SKIP
