import unittest

from flm.__main__ import make_args_parser


# ---------------------------------------------------------------------------
#  Command-line argument parsing
# ---------------------------------------------------------------------------

class TestCmdlineArgs(unittest.TestCase):

    def setUp(self):
        self.parser = make_args_parser()

    #
    # --validate-config-only and --print-config-json-schema are simple flags;
    # they must not swallow the input file name that follows them (which used
    # to leave `files` empty, so that the program would then block reading
    # standard input).
    #

    def test_validate_config_only_does_not_swallow_filename(self):
        args = self.parser.parse_args(['--validate-config-only', 'file.flm'])
        self.assertTrue(args.validate_config_only)
        self.assertEqual(args.files, ['file.flm'])

    def test_print_config_json_schema_does_not_swallow_filename(self):
        args = self.parser.parse_args(['--print-config-json-schema', 'file.flm'])
        self.assertTrue(args.print_config_json_schema)
        self.assertEqual(args.files, ['file.flm'])

    def test_flags_default_to_false(self):
        args = self.parser.parse_args(['file.flm'])
        self.assertFalse(args.validate_config_only)
        self.assertFalse(args.print_config_json_schema)

    def test_flags_after_filename(self):
        args = self.parser.parse_args(['file.flm', '--validate-config-only'])
        self.assertTrue(args.validate_config_only)
        self.assertEqual(args.files, ['file.flm'])

    def test_print_config_json_schema_with_output(self):
        args = self.parser.parse_args(
            ['--print-config-json-schema', 'file.flm', '-o', 'schema.json']
        )
        self.assertTrue(args.print_config_json_schema)
        self.assertEqual(args.files, ['file.flm'])
        self.assertEqual(args.output, 'schema.json')

    def test_flags_reject_explicit_value(self):
        with self.assertRaises(SystemExit):
            self.parser.parse_args(['--validate-config-only=full', 'file.flm'])

    #
    # --print-merged-config, in contrast, does take an optional value.
    #

    def test_print_merged_config_takes_optional_value(self):
        args = self.parser.parse_args(['--print-merged-config', 'full', 'file.flm'])
        self.assertEqual(args.print_merged_config, 'full')
        self.assertEqual(args.files, ['file.flm'])

    def test_print_merged_config_defaults_to_run(self):
        args = self.parser.parse_args(['file.flm', '--print-merged-config'])
        self.assertEqual(args.print_merged_config, 'run')
        self.assertEqual(args.files, ['file.flm'])


if __name__ == '__main__':
    unittest.main()
