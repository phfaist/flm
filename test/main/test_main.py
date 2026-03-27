import unittest

import io

from flm.main.main import (
    main,
    Main,
    ResourceAccessor,
    _process_arg_inline_configs,
    _TrivialContextManager,
    load_external_configs,
    main_print_merged_config,
)


# ---------------------------------------------------------------------------
#  _process_arg_inline_configs
# ---------------------------------------------------------------------------

class TestProcessArgInlineConfigs(unittest.TestCase):

    def test_none_input(self):
        self.assertIsNone(_process_arg_inline_configs(None))

    def test_string_json(self):
        self.assertEqual(_process_arg_inline_configs('{"a": 1}'), [{'a': 1}])

    def test_string_yaml(self):
        self.assertEqual(_process_arg_inline_configs('a: 1'), [{'a': 1}])

    def test_dict_input(self):
        self.assertEqual(_process_arg_inline_configs({'b': 2}), [{'b': 2}])

    def test_list_of_dicts(self):
        self.assertEqual(
            _process_arg_inline_configs([{'c': 3}, {'d': 4}]),
            [{'c': 3}, {'d': 4}]
        )

    def test_list_of_strings(self):
        self.assertEqual(
            _process_arg_inline_configs(['{"e": 5}', 'f: 6']),
            [{'e': 5}, {'f': 6}]
        )

    def test_empty_list(self):
        self.assertIsNone(_process_arg_inline_configs([]))

    def test_list_with_none(self):
        self.assertIsNone(_process_arg_inline_configs([None]))

    def test_list_all_none(self):
        self.assertIsNone(_process_arg_inline_configs([None, None]))


# ---------------------------------------------------------------------------
#  _TrivialContextManager
# ---------------------------------------------------------------------------

class TestTrivialContextManager(unittest.TestCase):

    def test_context_manager(self):
        cm = _TrivialContextManager('hello')
        with cm as v:
            self.assertEqual(v, 'hello')
        self.assertEqual(cm.value, 'hello')

    def test_exit_no_error(self):
        cm = _TrivialContextManager(42)
        cm.__enter__()
        # __exit__ should not raise
        cm.__exit__(None, None, None)


# ---------------------------------------------------------------------------
#  ResourceAccessor
# ---------------------------------------------------------------------------

class TestResourceAccessor(unittest.TestCase):

    def test_get_full_path_with_dir(self):
        ra = ResourceAccessor()
        self.assertEqual(ra.get_full_path('/tmp', 'file.txt', 'test', {}),
                         '/tmp/file.txt')

    def test_get_full_path_no_dir(self):
        ra = ResourceAccessor()
        self.assertEqual(ra.get_full_path('', 'file.txt', 'test', {}), 'file.txt')

    def test_get_full_path_none_dir(self):
        ra = ResourceAccessor()
        self.assertEqual(ra.get_full_path(None, 'file.txt', 'test', {}), 'file.txt')

    def test_template_path_has_none_and_templates_dir(self):
        ra = ResourceAccessor()
        # First element is None (relative to FLM source), second is the templates dir
        self.assertIsNone(ra.template_path[0])
        self.assertTrue(ra.template_path[1].endswith('templates'))


# ---------------------------------------------------------------------------
#  load_external_configs
# ---------------------------------------------------------------------------

class TestLoadExternalConfigs(unittest.TestCase):

    def test_no_config_found(self):
        # When no config files exist, returns [{}]
        result = load_external_configs(
            '/tmp/nonexistent_flm_dir_xyz',
            arg_config=None, arg_format=None, arg_workflow=None,
        )
        self.assertEqual(result, [{}])

    def test_dict_config(self):
        # When arg_config is a dict, it is returned directly (no file I/O)
        result = load_external_configs(
            '/tmp', arg_config={'key': 'val'}, arg_format=None, arg_workflow=None,
        )
        self.assertEqual(result, [{'key': 'val'}])


# ---------------------------------------------------------------------------
#  Main.__init__
# ---------------------------------------------------------------------------

class TestMainInit(unittest.TestCase):

    def test_flm_content_attributes(self):
        m = Main(flm_content='Test content', format='html')
        self.assertEqual(m.flm_content, 'Test content')
        self.assertIsNone(m.dirname)
        self.assertIsNone(m.basename)
        self.assertEqual(m.jobname, 'unknown-jobname')
        self.assertEqual(m.frontmatter_metadata, {})
        self.assertEqual(m.line_number_offset, 0)

    def test_kwargs_stored(self):
        m = Main(flm_content='x', format='html', workflow='compile',
                 template='mytemplate')
        self.assertEqual(m.arg_format, 'html')
        self.assertEqual(m.arg_workflow, 'compile')
        self.assertEqual(m.arg_template, 'mytemplate')
        self.assertEqual(m.arg_flm_content, 'x')

    def test_error_both_files_and_content(self):
        with self.assertRaises(ValueError):
            Main(files=['somefile.txt'], flm_content='hello')

    def test_error_no_input(self):
        with self.assertRaises(ValueError):
            Main()

    def test_frontmatter_parsed(self):
        m = Main(
            flm_content="---\nflm:\n  parsing:\n    dollar_inline_math_mode: true\n---\nHello",
            format='html',
        )
        self.assertTrue('flm' in m.frontmatter_metadata)
        self.assertEqual(m.flm_content, 'Hello')

    def test_inline_config_string(self):
        m = Main(flm_content='Hello', format='html',
                 inline_config='{"flm": {}}')
        self.assertEqual(m.arg_inline_configs, [{'flm': {}}])

    def test_inline_default_config(self):
        m = Main(flm_content='Hello', format='html',
                 inline_default_config='{"flm": {}}')
        self.assertEqual(m.arg_inline_default_configs, [{'flm': {}}])

    def test_doc_metadata(self):
        m = Main(flm_content='Hello', format='html')
        self.assertEqual(m.doc_metadata['jobname'], 'unknown-jobname')
        self.assertIsNone(m.doc_metadata['filepath']['dirname'])
        self.assertIsNone(m.doc_metadata['filepath']['basename'])

    def test_flm_run_info(self):
        m = Main(flm_content='Hello', format='html')
        self.assertEqual(m.flm_run_info['outputformat'], 'html')
        self.assertIsNone(m.flm_run_info['cwd'])
        self.assertIsNone(m.flm_run_info['input_source'])


# ---------------------------------------------------------------------------
#  Main.make_run_object / Main.run
# ---------------------------------------------------------------------------

class TestMainRun(unittest.TestCase):

    maxDiff = None

    def test_make_run_object(self):
        m = Main(flm_content='Hello', format='html')
        run_obj = m.make_run_object()
        self.assertEqual(type(run_obj).__name__, 'Run')

    def test_skip_write_return_result(self):
        m = Main(flm_content=r'Hello \textbf{world}!', format='html')
        result = m.run(skip_write_return_result=True)
        self.assertEqual(sorted(result.keys()), ['result', 'result_info'])
        self.assertEqual(
            result['result'],
            'Hello <span class="textbf">world</span>!'
        )


# ---------------------------------------------------------------------------
#  main() convenience function — output formats
# ---------------------------------------------------------------------------

class TestRunMain(unittest.TestCase):

    maxDiff = None

    def test_simple_text(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content=r"Hello \emph{world}!  Looking great today.",
            format='text'
        )
        self.assertEqual(sout.getvalue(), "Hello world! Looking great today.\n")

    def test_simple_html(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content=r"Hello \emph{world}!  Looking great today.",
            format='html',
            minimal_document=False,
        )
        self.assertEqual(
            sout.getvalue(),
            """Hello <span class="textit">world</span>! Looking great today.\n"""
        )

    def test_simple_frontmatter(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content=(
                "---\n"
                "flm:\n"
                "  parsing:\n"
                "    comment_start: '##'\n"
                "    enable_comments: true\n"
                "    dollar_inline_math_mode: true\n"
                "    force_block_level: true\n"
                "---\n"
                r"Hello \emph{world}!  Let $x$ and $y$ be real numbers."
                " ## comments configured like this!" "\n"
            ),
            format='html',
            minimal_document=False,
            suppress_final_newline=True,
        )
        self.assertEqual(
            sout.getvalue(),
            '<p>Hello <span class="textit">world</span>!'
            ' Let <span class="inline-math">\\(x\\)</span>'
            ' and <span class="inline-math">\\(y\\)</span>'
            ' be real numbers.</p>'
        )

    def test_markdown_output(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content=r'Hello \textbf{world}!',
            format='markdown',
            suppress_final_newline=True,
        )
        self.assertEqual(sout.getvalue(), 'Hello **world**\\!')

    def test_latex_output(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content=r'Hello \textbf{world}!',
            format='latex',
            suppress_final_newline=True,
        )
        self.assertEqual(sout.getvalue(), 'Hello \\textbf{world}!\n\n% no-endnotes\n')

    def test_suppress_final_newline(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content='Hello',
            format='text',
            suppress_final_newline=True,
        )
        self.assertEqual(sout.getvalue(), 'Hello')

    def test_with_inline_config(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content='Hello',
            format='text',
            suppress_final_newline=True,
            inline_config='{"flm": {"parsing": {"comment_start": "%%"}}}',
        )
        self.assertEqual(sout.getvalue(), 'Hello')

    def test_with_inline_default_config(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content='Hello',
            format='text',
            suppress_final_newline=True,
            inline_default_config='{"flm": {}}',
        )
        self.assertEqual(sout.getvalue(), 'Hello')

    def test_frontmatter_dollar_math(self):
        sout = io.StringIO()
        main(
            output=sout,
            flm_content=(
                "---\nflm:\n  parsing:\n"
                "    dollar_inline_math_mode: true\n---\n"
                "Hello $x$!"
            ),
            format='html',
            suppress_final_newline=True,
        )
        self.assertEqual(
            sout.getvalue(),
            'Hello <span class="inline-math">\\(x\\)</span>!'
        )


# ---------------------------------------------------------------------------
#  main_print_merged_config
# ---------------------------------------------------------------------------

class TestMainPrintMergedConfig(unittest.TestCase):

    def test_available_keys(self):
        self.assertEqual(
            main_print_merged_config.available_keys,
            ['run', 'full', 'full-flm', 'workflow', 'template']
        )

    def test_invalid_key_raises(self):
        with self.assertRaises(ValueError):
            main_print_merged_config(
                flm_content='Hello', format='html',
                print_merged_config='invalid',
            )


if __name__ == '__main__':
    unittest.main()
