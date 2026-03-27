import unittest

from flm.main.template import (
    _ProxyDictVarConfig,
    _StrTemplate,
    _default_ifmarks,
    replace_ifmarks,
    TemplateEngineBase,
    OnlyContentTemplate,
    SimpleStringTemplate,
)


class MockResourceAccessor:
    def __init__(self, content=''):
        self.content = content
        self.last_filename = None

    def read_file(self, path, filename, what, **kwargs):
        self.last_filename = filename
        return self.content


# ---------------------------------------------------------------------------
# _ProxyDictVarConfig
# ---------------------------------------------------------------------------

class TestProxyDictVarConfig(unittest.TestCase):

    maxDiff = None

    def _make(self, config, **kwargs):
        return _ProxyDictVarConfig(config, _default_ifmarks, **kwargs)

    def test_basic_key(self):
        p = self._make({'title': 'Hello'})
        self.assertEqual(p['title'], 'Hello')

    def test_nested_key(self):
        p = self._make({'nested': {'key': 'val'}})
        self.assertEqual(p['nested.key'], 'val')

    def test_missing_key_returns_empty_string(self):
        p = self._make({})
        self.assertEqual(p['missing'], '')

    def test_none_value_returns_empty_string(self):
        p = self._make({'x': None})
        self.assertEqual(p['x'], '')

    def test_deep_missing_returns_empty_string(self):
        p = self._make({'a': {'b': 'c'}})
        self.assertEqual(p['a.b.deep'], '')

    def test_none_as_empty_string_false(self):
        p = self._make({'x': None}, none_as_empty_string=False)
        self.assertIsNone(p['x'])

    def test_missing_none_as_empty_string_false(self):
        p = self._make({}, none_as_empty_string=False)
        self.assertIsNone(p['missing'])

    def test_if_true(self):
        p = self._make({'flag': True})
        self.assertEqual(p['if:flag'], _default_ifmarks['iftrue'])

    def test_if_false(self):
        p = self._make({'flag': False})
        self.assertEqual(p['if:flag'], _default_ifmarks['iffalse'])

    def test_if_missing_is_false(self):
        p = self._make({})
        self.assertEqual(p['if:missing'], _default_ifmarks['iffalse'])

    def test_else_key(self):
        p = self._make({})
        self.assertEqual(p['else'], _default_ifmarks['else'])

    def test_endif_key(self):
        p = self._make({})
        self.assertEqual(p['endif'], _default_ifmarks['endif'])


# ---------------------------------------------------------------------------
# _StrTemplate
# ---------------------------------------------------------------------------

class TestStrTemplate(unittest.TestCase):

    def test_basic_substitution(self):
        tpl = _StrTemplate('Hello ${name}!')
        self.assertEqual(tpl.substitute({'name': 'World'}), 'Hello World!')

    def test_dotted_key(self):
        tpl = _StrTemplate('${nested.key}')
        self.assertEqual(tpl.substitute({'nested.key': 'OK'}), 'OK')

    def test_colon_key(self):
        tpl = _StrTemplate('${if:flag}')
        self.assertEqual(tpl.substitute({'if:flag': 'MARK'}), 'MARK')

    def test_dash_key(self):
        tpl = _StrTemplate('${my-key}')
        self.assertEqual(tpl.substitute({'my-key': 'val'}), 'val')


# ---------------------------------------------------------------------------
# replace_ifmarks
# ---------------------------------------------------------------------------

class TestReplaceIfmarks(unittest.TestCase):

    maxDiff = None

    def setUp(self):
        self.ifm = _default_ifmarks

    def test_iftrue_no_else(self):
        c = 'before' + self.ifm['iftrue'] + 'SHOWN' + self.ifm['endif'] + 'after'
        self.assertEqual(replace_ifmarks(c, self.ifm), 'beforeSHOWNafter')

    def test_iffalse_no_else(self):
        c = 'before' + self.ifm['iffalse'] + 'HIDDEN' + self.ifm['endif'] + 'after'
        self.assertEqual(replace_ifmarks(c, self.ifm), 'beforeafter')

    def test_iftrue_with_else(self):
        c = 'A' + self.ifm['iftrue'] + 'YES' + self.ifm['else'] + 'NO' + self.ifm['endif'] + 'B'
        self.assertEqual(replace_ifmarks(c, self.ifm), 'AYESB')

    def test_iffalse_with_else(self):
        c = 'A' + self.ifm['iffalse'] + 'YES' + self.ifm['else'] + 'NO' + self.ifm['endif'] + 'B'
        self.assertEqual(replace_ifmarks(c, self.ifm), 'ANOB')

    def test_nested_conditions(self):
        inner = (self.ifm['iffalse'] + 'INNER_YES' + self.ifm['else']
                 + 'INNER_NO' + self.ifm['endif'])
        c = self.ifm['iftrue'] + 'OUTER(' + inner + ')' + self.ifm['endif']
        self.assertEqual(replace_ifmarks(c, self.ifm), 'OUTER(INNER_NO)')

    def test_no_marks(self):
        self.assertEqual(replace_ifmarks('plain text', self.ifm), 'plain text')

    def test_empty_string(self):
        self.assertEqual(replace_ifmarks('', self.ifm), '')

    def test_unmatched_if_raises(self):
        c = 'before' + self.ifm['iftrue'] + 'no endif'
        with self.assertRaises(ValueError):
            replace_ifmarks(c, self.ifm)


# ---------------------------------------------------------------------------
# TemplateEngineBase & OnlyContentTemplate
# ---------------------------------------------------------------------------

class TestTemplateEngineBase(unittest.TestCase):

    def test_attributes(self):
        t = TemplateEngineBase(
            template_info_path='/p',
            template_info_file='f.yaml',
            flm_run_info={'x': 1},
            document_template=None,
            template_engine_config={'a': 'b'},
        )
        self.assertEqual(t.template_info_path, '/p')
        self.assertEqual(t.template_info_file, 'f.yaml')
        self.assertEqual(t.template_engine_config, {'a': 'b'})

    def test_dispatch_initialize_calls_initialize(self):
        t = TemplateEngineBase(
            template_info_path='/p',
            template_info_file='f.yaml',
            flm_run_info={},
            document_template=None,
            template_engine_config={},
        )
        # Should not raise
        t.dispatch_initialize()


class TestOnlyContentTemplate(unittest.TestCase):

    def test_render_returns_content(self):
        t = OnlyContentTemplate(
            template_info_path='/tmp',
            template_info_file='test.yaml',
            flm_run_info={},
            document_template=None,
            template_engine_config={},
        )
        self.assertEqual(
            t.render_template({'content': '<h1>Hi</h1>'}),
            '<h1>Hi</h1>'
        )


# ---------------------------------------------------------------------------
# SimpleStringTemplate
# ---------------------------------------------------------------------------

class TestSimpleStringTemplate(unittest.TestCase):

    maxDiff = None

    def _make(self, template_content, template_info_file='test.yaml', **engine_config):
        ra = MockResourceAccessor(template_content)
        t = SimpleStringTemplate(
            template_info_path='/tmp',
            template_info_file=template_info_file,
            flm_run_info={'resource_accessor': ra},
            document_template=None,
            template_engine_config=engine_config,
        )
        t.dispatch_initialize()
        return t, ra

    def test_basic_render(self):
        t, _ = self._make('Title: ${title}')
        self.assertEqual(t.render_template({'title': 'Hello'}), 'Title: Hello')

    def test_if_true_render(self):
        t, _ = self._make('${if:show}YES${else}NO${endif}')
        self.assertEqual(t.render_template({'show': True}), 'YES')

    def test_if_false_render(self):
        t, _ = self._make('${if:show}YES${else}NO${endif}')
        self.assertEqual(t.render_template({'show': False}), 'NO')

    def test_if_missing_render(self):
        t, _ = self._make('${if:show}YES${else}NO${endif}')
        self.assertEqual(t.render_template({}), 'NO')

    def test_default_filename_html(self):
        _, ra = self._make('x', template_info_file='mytemplate.yaml')
        self.assertEqual(ra.last_filename, 'mytemplate.html')

    def test_custom_extension(self):
        _, ra = self._make('x', template_info_file='mytemplate.yaml',
                           template_content_extension='.txt')
        self.assertEqual(ra.last_filename, 'mytemplate.txt')

    def test_custom_filename(self):
        _, ra = self._make('x', template_info_file='mytemplate.yaml',
                           template_content_filename='custom.tpl')
        self.assertEqual(ra.last_filename, 'custom.tpl')

    def test_combined_substitution_and_ifmarks(self):
        t, _ = self._make('Hello ${name}, ${if:flag}active${endif}!')
        self.assertEqual(
            t.render_template({'name': 'World', 'flag': True}),
            'Hello World, active!'
        )


if __name__ == '__main__':
    unittest.main()
