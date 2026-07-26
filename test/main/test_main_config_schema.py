import unittest
import logging
import json
import io
import contextlib

from flm.main.run import (
    validate_config_for_schema,
    _collect_leaf_errors,
    _format_leaf_error,
    get_config_json_schema,
)

from flm.main.main import (
    Main,
    main_validate_config,
    main_print_config_json_schema,
)

import jsonschema


# ---------------------------------------------------------------------------
#  _collect_leaf_errors / _format_leaf_error
# ---------------------------------------------------------------------------

class TestCollectLeafErrors(unittest.TestCase):

    def _get_errors(self, schema, instance):
        v = jsonschema.Draft202012Validator(schema)
        return list(v.iter_errors(instance=instance))

    def test_simple_type_error(self):
        errors = self._get_errors({'type': 'integer'}, 'hello')
        self.assertEqual(len(errors), 1)
        leaves = _collect_leaf_errors(errors[0])
        self.assertEqual(len(leaves), 1)
        self.assertEqual(leaves[0].validator, 'type')

    def test_additional_properties(self):
        schema = {
            'type': 'object',
            'properties': {'x': {'type': 'integer'}},
            'additionalProperties': False,
        }
        errors = self._get_errors(schema, {'x': 1, 'y': 2})
        self.assertEqual(len(errors), 1)
        leaves = _collect_leaf_errors(errors[0])
        self.assertEqual(len(leaves), 1)
        self.assertTrue('y' in leaves[0].message)

    def test_anyof_drills_into_deeper_branch(self):
        schema = {
            'anyOf': [
                {'type': 'null'},
                {
                    'type': 'object',
                    'properties': {'x': {'type': 'integer'}},
                    'additionalProperties': False,
                },
            ]
        }
        errors = self._get_errors(schema, {'x': 'hello'})
        self.assertEqual(len(errors), 1)
        leaves = _collect_leaf_errors(errors[0])
        # Should drill past the anyOf into the object branch, finding the
        # type error on 'x' rather than the shallow "not null" error
        found_x = False
        for leaf in leaves:
            if 'x' in list(leaf.absolute_path):
                found_x = True
        self.assertTrue(found_x)

    def test_anyof_leaf_summarized(self):
        """When all sub-errors of anyOf are shallow type checks, return the
        anyOf itself as a leaf so _format_leaf_error can summarize types."""
        schema = {'anyOf': [{'type': 'null'}, {'type': 'array'}]}
        errors = self._get_errors(schema, 'hello')
        self.assertEqual(len(errors), 1)
        leaves = _collect_leaf_errors(errors[0])
        self.assertEqual(len(leaves), 1)
        self.assertEqual(leaves[0].validator, 'anyOf')

    def test_anyof_multiple_deep_errors(self):
        """Multiple deep errors within a single anyOf branch should all appear."""
        schema = {
            'anyOf': [
                {'type': 'null'},
                {
                    'type': 'object',
                    'properties': {
                        'a': {'type': 'integer'},
                        'b': {'type': 'integer'},
                    },
                    'additionalProperties': False,
                },
            ]
        }
        errors = self._get_errors(schema, {'a': 'x', 'b': 'y'})
        self.assertEqual(len(errors), 1)
        leaves = _collect_leaf_errors(errors[0])
        paths = [list(leaf.absolute_path) for leaf in leaves]
        self.assertTrue(['a'] in paths)
        self.assertTrue(['b'] in paths)


class TestFormatLeafError(unittest.TestCase):

    def _get_errors(self, schema, instance):
        v = jsonschema.Draft202012Validator(schema)
        return list(v.iter_errors(instance=instance))

    def test_simple_type_error_format(self):
        errors = self._get_errors(
            {'type': 'object', 'properties': {'x': {'type': 'integer'}}},
            {'x': 'bad'},
        )
        leaves = _collect_leaf_errors(errors[0])
        formatted = _format_leaf_error(leaves[0])
        self.assertTrue('$.x' in formatted)
        self.assertTrue('integer' in formatted)

    def test_root_path(self):
        errors = self._get_errors({'type': 'integer'}, 'bad')
        formatted = _format_leaf_error(errors[0])
        self.assertTrue('$:' in formatted)

    def test_anyof_leaf_summarizes_types(self):
        schema = {'anyOf': [{'type': 'null'}, {'type': 'array'}]}
        errors = self._get_errors(schema, 'hello')
        leaves = _collect_leaf_errors(errors[0])
        formatted = _format_leaf_error(leaves[0])
        self.assertTrue('null' in formatted)
        self.assertTrue('array' in formatted)
        self.assertTrue('expected' in formatted)


# ---------------------------------------------------------------------------
#  validate_config_for_schema
# ---------------------------------------------------------------------------

class TestValidateConfigForSchema(unittest.TestCase):

    def test_valid_config_no_warnings(self):
        schema = {
            'type': 'object',
            'properties': {'name': {'type': 'string'}},
        }
        with self.assertLogs('flm.main.run', level='DEBUG') as cm:
            logging.getLogger('flm.main.run').debug('sentinel')
            validate_config_for_schema('test', schema, {'name': 'hello'})
        # Only the sentinel log should appear, no warnings
        warning_logs = [r for r in cm.output if 'validation error' in r]
        self.assertEqual(len(warning_logs), 0)

    def test_invalid_config_emits_warning(self):
        schema = {
            'type': 'object',
            'properties': {'count': {'type': 'integer'}},
        }
        with self.assertLogs('flm.main.run', level='WARNING') as cm:
            validate_config_for_schema('mytest', schema, {'count': 'bad'})
        joined = '\n'.join(cm.output)
        self.assertTrue('mytest' in joined)
        self.assertTrue('integer' in joined)

    def test_multiple_errors_all_reported(self):
        schema = {
            'type': 'object',
            'properties': {
                'a': {'type': 'integer'},
                'b': {'type': 'integer'},
            },
        }
        with self.assertLogs('flm.main.run', level='WARNING') as cm:
            validate_config_for_schema('multi', schema, {'a': 'x', 'b': 'y'})
        joined = '\n'.join(cm.output)
        self.assertTrue('$.a' in joined)
        self.assertTrue('$.b' in joined)

    def test_anyof_error_concise(self):
        schema = {
            'type': 'object',
            'properties': {
                'val': {'anyOf': [{'type': 'null'}, {'type': 'array'}]},
            },
        }
        with self.assertLogs('flm.main.run', level='WARNING') as cm:
            validate_config_for_schema('anyoftest', schema, {'val': 123})
        joined = '\n'.join(cm.output)
        # Should contain the summarized "expected null or array" message
        self.assertTrue('expected' in joined)
        self.assertTrue('null' in joined)
        self.assertTrue('array' in joined)


# ---------------------------------------------------------------------------
#  get_config_json_schema (module-level function)
# ---------------------------------------------------------------------------

class TestGetConfigJsonSchema(unittest.TestCase):

    def test_returns_valid_json_schema(self):
        from flm.feature.math import FeatureClass as MathFeature
        schema = get_config_json_schema(
            feature_classes={'math': MathFeature},
            renderer_classes={},
            workflow_classes={},
        )
        self.assertEqual(schema.get('type'), 'object')
        self.assertTrue('properties' in schema)
        self.assertTrue('flm' in schema['properties'])

    def test_features_appear_in_schema(self):
        from flm.feature.math import FeatureClass as MathFeature
        from flm.feature.href import FeatureClass as HrefFeature
        schema = get_config_json_schema(
            feature_classes={'math': MathFeature, 'href': HrefFeature},
            renderer_classes={},
            workflow_classes={},
        )
        feature_props = (
            schema['properties']['flm']['properties']['features']['properties']
        )
        self.assertTrue('math' in feature_props)
        self.assertTrue('href' in feature_props)

    def test_schema_validates_valid_config(self):
        from flm.feature.math import FeatureClass as MathFeature
        schema = get_config_json_schema(
            feature_classes={'math': MathFeature},
            renderer_classes={},
            workflow_classes={},
        )
        v = jsonschema.Draft202012Validator(schema)
        # A minimal valid config should pass validation
        self.assertTrue(v.is_valid({'flm': {}}))

    def test_schema_is_json_serializable(self):
        from flm.feature.math import FeatureClass as MathFeature
        schema = get_config_json_schema(
            feature_classes={'math': MathFeature},
            renderer_classes={},
            workflow_classes={},
        )
        dumped = json.dumps(schema)
        reloaded = json.loads(dumped)
        self.assertTrue(isinstance(reloaded, dict))
        self.assertTrue('properties' in reloaded)

    def test_workflow_config_is_an_object_schema(self):
        from flm.main.workflow.templatebasedworkflow import (
            RenderWorkflowClass as TemplateBasedWorkflow
        )
        schema = get_config_json_schema(
            feature_classes={},
            renderer_classes={},
            workflow_classes={'templatebasedworkflow': TemplateBasedWorkflow},
        )
        workflow_config = (
            schema['properties']['flm']['properties']['workflow_config']
        )
        # must be a subschema object, not a list/tuple of them
        self.assertTrue(isinstance(workflow_config, dict))
        self.assertEqual(workflow_config['type'], 'object')
        self.assertTrue(
            'templatebasedworkflow' in workflow_config['properties']
        )

    def test_schema_keys_are_normalized(self):
        # The generated schema must not depend on the order in which the
        # feature/renderer/workflow classes are handed to us -- callers may
        # well collect those names in a set, whose iteration order varies
        # from one interpreter run to the next.
        from flm.feature.math import FeatureClass as MathFeature
        from flm.feature.href import FeatureClass as HrefFeature
        from flm.feature.defterm import FeatureClass as DeftermFeature

        def _make(feature_classes):
            return get_config_json_schema(
                feature_classes=feature_classes,
                renderer_classes={},
                workflow_classes={},
            )

        schema_a = _make({
            'math': MathFeature, 'href': HrefFeature, 'defterm': DeftermFeature,
        })
        schema_b = _make({
            'defterm': DeftermFeature, 'math': MathFeature, 'href': HrefFeature,
        })

        keys_a = list(
            schema_a['properties']['flm']['properties']['features']['properties'].keys()
        )
        self.assertEqual(keys_a, ['defterm', 'href', 'math'])
        self.assertEqual(json.dumps(schema_a), json.dumps(schema_b))


# ---------------------------------------------------------------------------
#  main_validate_config / main_print_config_json_schema (high-level entry points)
# ---------------------------------------------------------------------------

class TestMainValidateConfig(unittest.TestCase):

    def test_valid_content(self):
        # Should complete without error
        main_validate_config(
            flm_content='Hello',
            format='html',
        )

    def test_invalid_config_warns(self):
        with self.assertLogs('flm.main.run', level='WARNING') as cm:
            main_validate_config(
                flm_content=(
                    "---\n"
                    "flm:\n"
                    "  parsing:\n"
                    "    dollar_inline_math_mode: not-a-bool\n"
                    "---\n"
                    "Hello"
                ),
                format='html',
            )
        joined = '\n'.join(cm.output)
        self.assertTrue('validation error' in joined)


class TestMainPrintConfigJsonSchema(unittest.TestCase):

    def test_prints_valid_json(self):
        captured = []
        main_print_config_json_schema(
            flm_content='Hello',
            format='html',
            _print_fn=captured.append,
        )
        schema = json.loads(captured[0])
        self.assertEqual(schema.get('type'), 'object')
        self.assertTrue('properties' in schema)

    def test_schema_includes_features(self):
        captured = []
        main_print_config_json_schema(
            flm_content='Hello',
            format='html',
            _print_fn=captured.append,
        )
        schema = json.loads(captured[0])
        flm_props = schema['properties']['flm']['properties']
        self.assertTrue('features' in flm_props)
        self.assertTrue('parsing' in flm_props)

    def test_output_is_reproducible(self):
        captured = []
        for _ in range(2):
            main_print_config_json_schema(
                flm_content='Hello',
                format='html',
                _print_fn=captured.append,
            )
        self.assertEqual(captured[0], captured[1])
        # keys are emitted in sorted order
        self.assertTrue('"additionalProperties"' in captured[0])
        schema = json.loads(captured[0])
        renderer_keys = list(
            schema['properties']['flm']['properties']['renderer']['properties'].keys()
        )
        self.assertEqual(renderer_keys, sorted(renderer_keys))

    def test_writes_to_output_file_object(self):
        fout = io.StringIO()
        main_print_config_json_schema(
            flm_content='Hello',
            format='html',
            output=fout,
        )
        schema = json.loads(fout.getvalue())
        self.assertEqual(schema.get('type'), 'object')

    def test_writes_to_stdout_by_default(self):
        fout = io.StringIO()
        with contextlib.redirect_stdout(fout):
            main_print_config_json_schema(
                flm_content='Hello',
                format='html',
            )
        schema = json.loads(fout.getvalue())
        self.assertEqual(schema.get('type'), 'object')


if __name__ == '__main__':
    unittest.main()
