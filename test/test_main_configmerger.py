import unittest

### BEGIN_TEST_FLM_SKIP

from flm.main.configmerger import ConfigMerger

class TestRecursiveAssignDefaults(unittest.TestCase):

    maxDiff = None

    def test_simple(self):

        d = ConfigMerger().recursive_assign_defaults([
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

        d = ConfigMerger().recursive_assign_defaults([
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


    def test_with_nomerge(self):

        d = ConfigMerger().recursive_assign_defaults([
            {
                'k1': { 'A': 1, 'B': 2 },
            },
            {
                'k1': { '$no-merge': True, 'A': 'A-DEFAULT', 'B': 'B-DEFAULT' },
            },
            {
                'k1': { 'C': 'ignore-past-no-merge' }
            },
        ])

        self.assertEqual(d, {
            'k1': { 'A': 1, 'B': 2 }
        })

    def test_with_nomerge_2(self):

        d = ConfigMerger().recursive_assign_defaults([
            {
                'k1': { },
            },
            {
                'k1': { '$no-merge': True, 'A': 'A-DEFAULT', 'B': 'B-DEFAULT' },
            },
            {
                'k1': { 'C': 'ignore-past-no-merge' }
            },
        ])

        self.assertEqual(d, {
            'k1': { 'A': 'A-DEFAULT', 'B': 'B-DEFAULT' }
        })

    def test_with_nomerge_3(self):

        d = ConfigMerger().recursive_assign_defaults([
            {
            },
            {
                'k1': { '$no-merge': True, 'A': 'A-DEFAULT', 'B': 'B-DEFAULT' },
            },
            {
                'k1': { 'C': 'ignore-past-no-merge' }
            },
        ])

        self.assertEqual(d, {
            'k1': { 'A': 'A-DEFAULT', 'B': 'B-DEFAULT' }
        })


    def test_with_nomerge_4(self):

        d = ConfigMerger().recursive_assign_defaults([
            {
                'k1': None,
            },
            {
                'k1': { '$no-merge': True, 'A': 'A-DEFAULT', 'B': 'B-DEFAULT' },
            },
            {
                'k1': { 'C': 'ignore-past-no-merge' }
            },
        ])

        self.assertEqual(d, {
            'k1': None
        })


    def test_with_nomerge_rec(self):

        d = ConfigMerger().recursive_assign_defaults([
            {
                'k1': { 'A': 1, 'B': { 'bbb': '222' } },
            },
            {
                'k1': {
                    '$no-merge': True,
                    'A': 'A-DEFAULT', 'B': { 'ccc': '333' },
                },
            },
            {
                'k1': { 'B': { 'ddd': 'ignore-past-no-merge' } }
            },
        ])

        self.assertEqual(d, {
            'k1': { 'A': 1, 'B': { 'bbb': '222' } }
        })



    # -- test presets --

    def test_preset_defaults(self):

        d = ConfigMerger().recursive_assign_defaults([
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
        
        d = ConfigMerger().recursive_assign_defaults([
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
        
        d = ConfigMerger().recursive_assign_defaults([
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

### END_TEST_FLM_SKIP
