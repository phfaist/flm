import unittest

from flm.main.configmerger import (
    ConfigMerger,
    ListProperty,
    PresetKeepMarker,
    PresetDefaults,
    PresetMergeConfig,
    PresetRemoveItem,
    PresetImport,
    _get_preset_keyvals,
    get_default_presets,
)


class TestHelpers(unittest.TestCase):

    def test_get_preset_keyvals_with_presets(self):
        result = _get_preset_keyvals({'$a': 1, 'b': 2, '$c': 3})
        self.assertEqual(result, [('$a', 1), ('$c', 3)])

    def test_get_preset_keyvals_non_mapping(self):
        self.assertEqual(_get_preset_keyvals('hello'), [])

    def test_get_preset_keyvals_empty(self):
        self.assertEqual(_get_preset_keyvals({}), [])

    def test_get_preset_keyvals_no_presets(self):
        self.assertEqual(_get_preset_keyvals({'a': 1, 'b': 2}), [])

    def test_list_property_marker(self):
        lp = ListProperty()
        self.assertTrue(isinstance(lp, ListProperty))

    def test_get_default_presets(self):
        presets = get_default_presets()
        self.assertTrue('$defaults' in presets)
        self.assertTrue('$merge-config' in presets)
        self.assertTrue('$remove-item' in presets)
        self.assertTrue('$import' in presets)
        self.assertTrue('$_cwd' in presets)


class TestConfigMergerInit(unittest.TestCase):

    def test_default_presets(self):
        cm = ConfigMerger()
        self.assertTrue('$defaults' in cm.presets)
        self.assertTrue('$merge-config' in cm.presets)

    def test_custom_presets(self):
        cm = ConfigMerger(presets={'$custom': None})
        self.assertEqual(cm.presets, {'$custom': None})

    def test_defaults_additional_sources(self):
        cm = ConfigMerger(defaults_additional_sources=None)
        self.assertTrue('$defaults' in cm.presets)


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


class TestEmptyAndEdgeCases(unittest.TestCase):

    maxDiff = None

    def test_empty_obj_list_dict(self):
        self.assertEqual(ConfigMerger().recursive_assign_defaults([]), {})

    def test_empty_obj_list_list(self):
        self.assertEqual(
            ConfigMerger().recursive_assign_defaults_list([], []),
            []
        )

    def test_none_filtered_in_list(self):
        result = ConfigMerger().recursive_assign_defaults_list(
            [None, [1, 2], None, [3]], []
        )
        self.assertEqual(result, [1, 2])

    def test_scalar_first_wins(self):
        d = ConfigMerger().recursive_assign_defaults([
            {'k': 42},
            {'k': 99},
        ])
        self.assertEqual(d, {'k': 42})

    def test_scalar_over_dict(self):
        d = ConfigMerger().recursive_assign_defaults([
            {'k': 42},
            {'k': {'a': 1}},
        ])
        self.assertEqual(d, {'k': 42})

    def test_list_first_wins_no_defaults(self):
        d = ConfigMerger().recursive_assign_defaults([
            {'k': ['a', 'b']},
            {'k': ['c', 'd', 'e']},
        ])
        self.assertEqual(d, {'k': ['a', 'b']})

    def test_non_mapping_ignored(self):
        d = ConfigMerger().recursive_assign_defaults([
            {'a': 1},
            'not-a-dict',
            {'b': 2},
        ])
        self.assertEqual(d, {'a': 1, 'b': 2})

    def test_single_obj(self):
        d = ConfigMerger().recursive_assign_defaults([
            {'x': 1, 'y': {'a': 'A'}},
        ])
        self.assertEqual(d, {'x': 1, 'y': {'a': 'A'}})


class TestPresetKeepMarker(unittest.TestCase):

    maxDiff = None

    def test_cwd_marker_kept(self):
        d = ConfigMerger().recursive_assign_defaults([
            {'$_cwd': '/tmp', 'a': 1},
        ])
        self.assertEqual(d, {'a': 1, '$_cwd': '/tmp'})


class TestPresetErrors(unittest.TestCase):

    def test_nomerge_invalid_value(self):
        with self.assertRaises(ValueError):
            ConfigMerger().recursive_assign_defaults([
                {'k': {'$no-merge': False}},
            ])

    def test_remove_item_no_name(self):
        with self.assertRaises(ValueError):
            ConfigMerger().recursive_assign_defaults([
                {'k': [{'$remove-item': None}]},
            ])

    def test_remove_item_nonexistent(self):
        with self.assertRaises(ValueError):
            ConfigMerger().recursive_assign_defaults([
                {'k': [{'name': 'A'}, {'$remove-item': 'B'}]},
            ])

    def test_merge_config_no_name(self):
        with self.assertRaises(ValueError):
            ConfigMerger().recursive_assign_defaults([
                {'k': [{'name': 'A'}, {'$merge-config': {}}]},
            ])

    def test_merge_config_nonexistent(self):
        with self.assertRaises(ValueError):
            ConfigMerger().recursive_assign_defaults([
                {'k': [{'name': 'A'}, {'$merge-config': {'name': 'B'}}]},
            ])

    def test_multiple_preset_keys_in_list_item(self):
        with self.assertRaises(ValueError):
            ConfigMerger().recursive_assign_defaults([
                {'k': [{'$defaults': True, '$remove-item': 'X'}]},
            ])


class TestMergeConfigOverwrite(unittest.TestCase):

    maxDiff = None

    def test_merge_config_overwrites_existing_key(self):
        d = ConfigMerger().recursive_assign_defaults([
            {
                'items': [
                    {'$defaults': True},
                    {'$merge-config': {
                        'name': 'A',
                        'config': {'v': 'new'},
                    }},
                ],
            },
            {
                'items': [
                    {'name': 'A', 'config': {'v': 'old', 'w': 3}},
                ],
            },
        ])
        self.assertEqual(d, {
            'items': [
                {'name': 'A', 'config': {'v': 'new', 'w': 3}},
            ],
        })

    def test_defaults_then_append(self):
        d = ConfigMerger().recursive_assign_defaults([
            {
                'items': [
                    {'name': 'X', 'config': {'x': 1}},
                    {'$defaults': True},
                ],
            },
            {
                'items': [
                    {'name': 'Y', 'config': {'y': 2}},
                ],
            },
        ])
        self.assertEqual(d, {
            'items': [
                {'name': 'X', 'config': {'x': 1}},
                {'name': 'Y', 'config': {'y': 2}},
            ],
        })


if __name__ == '__main__':
    unittest.main()
