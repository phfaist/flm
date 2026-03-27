import unittest

from flm.main.importclass import import_class


class TestImportClassDotted(unittest.TestCase):
    """Tests for import_class with dotted fullname (module.ClassName)."""

    def test_dotted_fullname(self):
        mod, cls = import_class('flm.fragmentrenderer.html.HtmlFragmentRenderer')
        self.assertEqual(mod.__name__, 'flm.fragmentrenderer.html')
        self.assertEqual(cls.__name__, 'HtmlFragmentRenderer')

    def test_dotted_fullname_with_default_classnames(self):
        # When fullname is dotted AND default_classnames is given,
        # tries default_classnames on fullname-as-module first
        mod, cls = import_class(
            'flm.fragmentrenderer.html',
            default_classnames=['HtmlFragmentRenderer']
        )
        self.assertEqual(mod.__name__, 'flm.fragmentrenderer.html')
        self.assertEqual(cls.__name__, 'HtmlFragmentRenderer')

    def test_dotted_default_classnames_first_wins(self):
        mod, cls = import_class(
            'os.path',
            default_classnames=['join', 'exists']
        )
        self.assertEqual(cls.__name__, 'join')

    def test_dotted_wrong_class_raises(self):
        with self.assertRaises(ValueError):
            import_class('flm.fragmentrenderer.html.NonExistentClass')

    def test_dotted_nonexistent_module_raises(self):
        with self.assertRaises(ValueError):
            import_class('nonexistent.module.ClassName')


class TestImportClassNoDot(unittest.TestCase):
    """Tests for import_class with bare name (no dot)."""

    def test_no_dot_no_default_classnames_raises(self):
        with self.assertRaises(ValueError):
            import_class('html')

    def test_no_dot_empty_default_classnames_raises(self):
        with self.assertRaises(ValueError):
            import_class('html', default_classnames=[])

    def test_no_dot_with_default_prefix(self):
        mod, cls = import_class(
            'html',
            default_classnames=['HtmlFragmentRenderer'],
            default_prefix='flm.fragmentrenderer'
        )
        self.assertEqual(mod.__name__, 'flm.fragmentrenderer.html')
        self.assertEqual(cls.__name__, 'HtmlFragmentRenderer')

    def test_no_dot_without_default_prefix(self):
        # Without default_prefix, tries bare name as module - 'html' stdlib
        # doesn't have 'HtmlFragmentRenderer', so raises
        with self.assertRaises(ValueError):
            import_class('html', default_classnames=['HtmlFragmentRenderer'])

    def test_no_dot_default_prefix_not_found_falls_back(self):
        # If prefixed module not found, falls back to bare module name
        # 'os' has 'getcwd'
        mod, cls = import_class(
            'os',
            default_classnames=['getcwd'],
            default_prefix='nonexistent.prefix'
        )
        self.assertEqual(mod.__name__, 'os')
        self.assertEqual(cls.__name__, 'getcwd')


class TestImportClassReturnValues(unittest.TestCase):
    """Tests for return value structure."""

    def test_returns_module_and_class(self):
        mod, cls = import_class('os.path.join')
        self.assertTrue(callable(cls))
        self.assertTrue(hasattr(mod, '__name__'))

    def test_returned_class_is_usable(self):
        mod, cls = import_class(
            'flm.fragmentrenderer.html.HtmlFragmentRenderer'
        )
        instance = cls()
        self.assertEqual(instance.__class__.__name__, 'HtmlFragmentRenderer')


if __name__ == '__main__':
    unittest.main()
