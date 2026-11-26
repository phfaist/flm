import io
import contextlib
import unittest

from flm.__main__ import (
    run_main,
    print_python_version_deprecation_warning,
)


class TestPythonVersionDeprecationWarning(unittest.TestCase):

    def test_print_warning(self):
        s = io.StringIO()
        print_python_version_deprecation_warning(stream=s)
        value = s.getvalue()
        self.assertTrue('no longer maintained' in value)
        self.assertTrue('https://github.com/phfaist/flm#install-flm' in value)

    def test_run_main_prints_warning_to_stderr_first(self):
        out = io.StringIO()
        err = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            with self.assertRaises(SystemExit):
                run_main(['--version'])
        self.assertTrue(
            err.getvalue().strip().startswith('⚠️  NOTICE: You are running the Python version')
        )
        self.assertFalse('NOTICE' in out.getvalue())


if __name__ == '__main__':
    unittest.main()
