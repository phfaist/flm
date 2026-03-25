import unittest

from flm.flmenvironment import make_standard_environment
from flm.stdfeatures import standard_features
from flm.fragmentrenderer.html import HtmlFragmentRenderer
from flm.counter import CounterFormatter, ValueWithSubNums

from flm.feature.numbering import (
    Counter,
    CounterAlias,
    FeatureNumbering,
    get_document_render_counter,
    _CounterIface,
    _DocCounterState,
    _DocCounterStateAliasCounter,
    _NumprefixAndValueForDocStateCompute,
)
from flm.feature.headings import FeatureHeadings


def _mk_formatter(fmt='arabic', prefix='', delimiters=('', ''),
                   counter_formatter_id='test'):
    return CounterFormatter(
        format_num=fmt,
        prefix_display=prefix,
        delimiters=delimiters,
        counter_formatter_id=counter_formatter_id,
    )


def mk_flm_environ_with_numbering(number_within=None, headings_depth=False):
    features = standard_features(headings=False)
    features.append(FeatureHeadings(numbering_section_depth=headings_depth))
    features.append(FeatureNumbering(number_within=number_within or {}))
    return make_standard_environment(features)


def _get_numbering_mgr(environ):
    frag = environ.make_fragment('Hello')
    doc = environ.make_document(frag.render)
    fr = HtmlFragmentRenderer()
    result, render_context = doc.render(fr)
    return render_context.feature_render_manager('numbering')


# ----------------------------------------------------------------
# Counter class
# ----------------------------------------------------------------

class TestCounter(unittest.TestCase):

    def test_initial_value_default(self):
        c = Counter(_mk_formatter())
        self.assertEqual(c.value, 0)

    def test_initial_value_custom(self):
        c = Counter(_mk_formatter(), initial_value=5)
        self.assertEqual(c.value, 5)

    def test_step(self):
        c = Counter(_mk_formatter())
        v = c.step()
        self.assertEqual(v, 1)
        self.assertEqual(c.value, 1)

    def test_step_multiple(self):
        c = Counter(_mk_formatter())
        c.step()
        c.step()
        v = c.step()
        self.assertEqual(v, 3)
        self.assertEqual(c.value, 3)

    def test_set_value(self):
        c = Counter(_mk_formatter())
        result = c.set_value(10)
        self.assertEqual(result, 10)
        self.assertEqual(c.value, 10)

    def test_format_flm_default(self):
        c = Counter(_mk_formatter())
        c.step()
        self.assertEqual(c.format_flm(), '1')

    def test_format_flm_explicit_value(self):
        c = Counter(_mk_formatter())
        self.assertEqual(c.format_flm(value=5), '5')

    def test_step_and_format_flm(self):
        c = Counter(_mk_formatter())
        val, fmt = c.step_and_format_flm()
        self.assertEqual(val, 1)
        self.assertEqual(fmt, '1')

    def test_step_and_format_flm_second(self):
        c = Counter(_mk_formatter())
        c.step()
        val, fmt = c.step_and_format_flm()
        self.assertEqual(val, 2)
        self.assertEqual(fmt, '2')

    def test_formatter_stored(self):
        cf = _mk_formatter()
        c = Counter(cf)
        self.assertIs(c.formatter, cf)

    def test_reset(self):
        c = Counter(_mk_formatter(), initial_value=0)
        c.step()
        c.step()
        c.reset()
        self.assertEqual(c.value, 0)


# ----------------------------------------------------------------
# CounterAlias class
# ----------------------------------------------------------------

class TestCounterAlias(unittest.TestCase):

    def test_value_reflects_original(self):
        cf = _mk_formatter()
        c = Counter(cf)
        c.step()
        c.step()
        alias = CounterAlias(_mk_formatter(fmt='roman'), c)
        self.assertEqual(alias.value, 2)

    def test_step_increments_original(self):
        cf = _mk_formatter()
        c = Counter(cf)
        c.step()
        alias = CounterAlias(_mk_formatter(fmt='roman'), c)
        alias.step()
        self.assertEqual(c.value, 2)
        self.assertEqual(alias.value, 2)

    def test_format_flm_uses_own_formatter(self):
        cf = _mk_formatter()
        c = Counter(cf)
        c.step()
        c.step()
        alias = CounterAlias(_mk_formatter(fmt='roman'), c)
        self.assertEqual(alias.format_flm(), 'ii')

    def test_format_flm_explicit_value(self):
        cf = _mk_formatter()
        c = Counter(cf)
        alias = CounterAlias(_mk_formatter(fmt='roman'), c)
        self.assertEqual(alias.format_flm(value=4), 'iv')

    def test_step_and_format_flm(self):
        cf = _mk_formatter()
        c = Counter(cf)
        alias = CounterAlias(_mk_formatter(fmt='roman'), c)
        val, fmt = alias.step_and_format_flm()
        self.assertEqual(val, 1)
        self.assertEqual(fmt, 'i')

    def test_formatter_independent(self):
        cf_arabic = _mk_formatter()
        cf_roman = _mk_formatter(fmt='roman')
        c = Counter(cf_arabic)
        alias = CounterAlias(cf_roman, c)
        self.assertIs(alias.formatter, cf_roman)
        self.assertIs(c.formatter, cf_arabic)


# ----------------------------------------------------------------
# _CounterIface class
# ----------------------------------------------------------------

class TestCounterIface(unittest.TestCase):

    def test_register_item_increments(self):
        cf = _mk_formatter()
        ci = _CounterIface('test', simple_counter=Counter(cf))
        info = ci.register_item()
        self.assertEqual(info['formatted_value'], '1')
        self.assertTrue(isinstance(info['value'], ValueWithSubNums))
        self.assertIsNone(info['numprefix'])

    def test_register_item_second(self):
        cf = _mk_formatter()
        ci = _CounterIface('test', simple_counter=Counter(cf))
        ci.register_item()
        info = ci.register_item()
        self.assertEqual(info['formatted_value'], '2')

    def test_register_item_custom_label(self):
        cf = _mk_formatter()
        ci = _CounterIface('test', simple_counter=Counter(cf))
        info = ci.register_item(custom_label='Custom')
        self.assertIsNone(info['value'])
        self.assertIsNone(info['numprefix'])
        self.assertEqual(info['formatted_value'], 'Custom')

    def test_counter_name_stored(self):
        cf = _mk_formatter()
        ci = _CounterIface('myname', simple_counter=Counter(cf))
        self.assertEqual(ci.counter_name, 'myname')

    def test_formatter_accessible(self):
        cf = _mk_formatter()
        c = Counter(cf)
        ci = _CounterIface('test', simple_counter=c)
        self.assertIs(ci.formatter, cf)


# ----------------------------------------------------------------
# get_document_render_counter (no numbering feature)
# ----------------------------------------------------------------

class TestGetDocumentRenderCounter(unittest.TestCase):

    def _fake_rc(self):
        class FakeRenderContext:
            def supports_feature(self, name):
                return False
        return FakeRenderContext()

    def test_returns_counter_iface(self):
        rc = self._fake_rc()
        cf = _mk_formatter()
        ci = get_document_render_counter(rc, 'eq', cf)
        self.assertTrue(isinstance(ci, _CounterIface))

    def test_counter_increments(self):
        rc = self._fake_rc()
        cf = _mk_formatter()
        ci = get_document_render_counter(rc, 'eq', cf)
        info1 = ci.register_item()
        self.assertEqual(info1['formatted_value'], '1')
        info2 = ci.register_item()
        self.assertEqual(info2['formatted_value'], '2')

    def test_always_number_within_raises(self):
        rc = self._fake_rc()
        cf = _mk_formatter()
        with self.assertRaises(ValueError):
            get_document_render_counter(
                rc, 'eq', cf,
                always_number_within={'reset_at': 'section'}
            )

    def test_alias_counter(self):
        rc = self._fake_rc()
        cf_arabic = _mk_formatter()
        cf_roman = _mk_formatter(fmt='roman', counter_formatter_id='roman')
        base = get_document_render_counter(rc, 'base', cf_arabic)
        base.register_item()
        base.register_item()
        alias = get_document_render_counter(rc, 'alias', cf_roman, alias_counter=base)
        self.assertEqual(alias.simple_counter.value, 2)
        info = alias.register_item()
        self.assertEqual(info['formatted_value'], 'iii')
        # Original is also incremented
        self.assertEqual(base.simple_counter.value, 3)


# ----------------------------------------------------------------
# FeatureNumbering init
# ----------------------------------------------------------------

class TestFeatureNumberingInit(unittest.TestCase):

    def test_feature_name(self):
        fn = FeatureNumbering()
        self.assertEqual(fn.feature_name, 'numbering')

    def test_default_number_within_empty(self):
        fn = FeatureNumbering()
        self.assertEqual(fn.number_within, {})

    def test_number_within_stored(self):
        nw = {'equation': {'reset_at': 'section', 'numprefix': '${section}.'}}
        fn = FeatureNumbering(number_within=nw)
        self.assertEqual(fn.number_within, nw)


# ----------------------------------------------------------------
# RenderManager
# ----------------------------------------------------------------

class TestFeatureNumberingRenderManager(unittest.TestCase):

    def test_register_counter(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        ci = nmgr.register_counter('mytest', cf)
        self.assertTrue(isinstance(ci, _DocCounterState))

    def test_register_counter_empty_name_raises(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        with self.assertRaises(ValueError):
            nmgr.register_counter('', cf)

    def test_register_counter_duplicate_raises(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        nmgr.register_counter('mytest', cf)
        with self.assertRaises(ValueError):
            nmgr.register_counter('mytest', cf)

    def test_register_counter_alias_and_always_number_within_raises(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        base = nmgr.register_counter('base', cf)
        with self.assertRaises(ValueError):
            nmgr.register_counter(
                'alias', cf,
                alias_counter=base,
                always_number_within={'reset_at': 'section'}
            )

    def test_register_item(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        ci = nmgr.register_counter('mytest', cf)
        info = ci.register_item()
        self.assertEqual(info['formatted_value'], '1')
        self.assertEqual(info['number'], 1)
        self.assertEqual(info['subnums'], ())
        self.assertIsNone(info['numprefix'])

    def test_register_item_increments(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        ci = nmgr.register_counter('mytest', cf)
        ci.register_item()
        info2 = ci.register_item()
        self.assertEqual(info2['formatted_value'], '2')
        self.assertEqual(info2['number'], 2)

    def test_register_item_custom_label(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        nmgr.register_counter('mytest', cf)
        info = nmgr.register_item('mytest', custom_label='Custom')
        self.assertIsNone(info['value'])
        self.assertIsNone(info['number'])
        self.assertIsNone(info['subnums'])
        self.assertIsNone(info['numprefix'])
        self.assertEqual(info['formatted_value'], 'Custom')

    def test_get_formatted_counter_value(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter(prefix='Eq.~', delimiters=('(', ')'))
        ci = nmgr.register_counter('mytest', cf)
        ci.register_item()
        fmtval = nmgr.get_formatted_counter_value('mytest', with_prefix=True)
        self.assertEqual(fmtval, 'Eq.~(1)')
        fmtval2 = nmgr.get_formatted_counter_value('mytest', with_prefix=False)
        self.assertEqual(fmtval2, '(1)')

    def test_alias_counter(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf_arabic = _mk_formatter()
        cf_roman = _mk_formatter(fmt='roman', counter_formatter_id='roman')
        base = nmgr.register_counter('base', cf_arabic)
        base.register_item()
        base.register_item()
        alias = nmgr.register_counter('alias_c', cf_roman, alias_counter=base)
        info = alias.register_item()
        self.assertEqual(info['formatted_value'], 'iii')
        fmtval = alias.get_formatted_counter_value(with_prefix=False)
        self.assertEqual(fmtval, 'iii')

    def test_invalid_number_within_missing_reset_at(self):
        environ = mk_flm_environ_with_numbering(
            number_within={'equation': {'numprefix': 'x'}}
        )
        with self.assertRaises(ValueError):
            _get_numbering_mgr(environ)


# ----------------------------------------------------------------
# RenderManager doc state management
# ----------------------------------------------------------------

class TestRenderManagerDocState(unittest.TestCase):

    def test_set_render_doc_state(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        nmgr.set_render_doc_state('cnt-section', '1')
        self.assertEqual(nmgr.render_doc_states['cnt-section'], '1')

    def test_set_render_doc_state_overwrite(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        nmgr.set_render_doc_state('cnt-section', '1')
        nmgr.set_render_doc_state('cnt-section', '2')
        self.assertEqual(nmgr.render_doc_states['cnt-section'], '2')

    def test_clear_self_upon_change(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        nmgr.set_render_doc_state('cnt-section', '1')
        nmgr.set_render_doc_state(
            'cnt-subsection', '1.1',
            clear_self_upon_change=['cnt-section']
        )
        self.assertEqual(nmgr.render_doc_states['cnt-subsection'], '1.1')
        # Changing section should clear subsection
        nmgr.set_render_doc_state('cnt-section', '2')
        self.assertIsNone(nmgr.render_doc_states.get('cnt-subsection'))

    def test_clear_render_doc_state(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        nmgr.set_render_doc_state('cnt-section', '1')
        nmgr.set_render_doc_state(
            'cnt-subsection', '1.1',
            clear_self_upon_change=['cnt-section']
        )
        nmgr.clear_render_doc_state('cnt-section')
        # Clearing section should also set subsection to None
        self.assertIsNone(nmgr.render_doc_states.get('cnt-subsection'))


# ----------------------------------------------------------------
# _number_within_parent_counters / compute_use_doc_state_keys
# ----------------------------------------------------------------

class TestParentCounters(unittest.TestCase):

    def test_no_parents(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        parents = nmgr._number_within_parent_counters('figure')
        self.assertEqual(parents, [])

    def test_one_parent(self):
        environ = mk_flm_environ_with_numbering(number_within={
            'myeq': {'reset_at': 'mysection', 'numprefix': ''},
        })
        nmgr = _get_numbering_mgr(environ)
        parents = nmgr._number_within_parent_counters('myeq')
        self.assertEqual(parents, ['mysection'])

    def test_chain_of_parents(self):
        environ = mk_flm_environ_with_numbering(number_within={
            'mysubsection': {'reset_at': 'mysection', 'numprefix': ''},
            'myeq': {'reset_at': 'mysubsection', 'numprefix': ''},
        })
        nmgr = _get_numbering_mgr(environ)
        parents = nmgr._number_within_parent_counters('myeq')
        self.assertEqual(parents, ['mysubsection', 'mysection'])

    def test_compute_use_doc_state_keys(self):
        environ = mk_flm_environ_with_numbering(number_within={
            'mysubsection': {'reset_at': 'mysection', 'numprefix': ''},
            'myeq': {'reset_at': 'mysubsection', 'numprefix': ''},
        })
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        nmgr.register_counter('mysection', cf)
        nmgr.register_counter('mysubsection', cf)
        nmgr.register_counter('myeq', cf)
        keys = nmgr.compute_use_doc_state_keys('myeq')
        self.assertEqual(keys, ['cnt-mysubsection', 'cnt-mysection'])

    def test_compute_use_doc_state_keys_no_parents(self):
        environ = mk_flm_environ_with_numbering()
        nmgr = _get_numbering_mgr(environ)
        cf = _mk_formatter()
        nmgr.register_counter('myfigure', cf)
        keys = nmgr.compute_use_doc_state_keys('myfigure')
        self.assertEqual(keys, [])


# ----------------------------------------------------------------
# _DocCounterState repr
# ----------------------------------------------------------------

class TestDocCounterStateRepr(unittest.TestCase):

    def test_repr_basic(self):
        cf = _mk_formatter()

        class FakeRdrMgr:
            render_doc_states = {}

        dcs = _DocCounterState(
            formatter=cf,
            rdr_mgr=FakeRdrMgr(),
            counter_name='equation',
            base_use_doc_state_keys=None,
            numprefix_and_value_for_doc_state=None,
        )
        self.assertEqual(repr(dcs), "_DocCounterState<\u2018equation\u2019 None>")

    def test_repr_with_base_keys(self):
        cf = _mk_formatter()

        class FakeRdrMgr:
            render_doc_states = {}

        dcs = _DocCounterState(
            formatter=cf,
            rdr_mgr=FakeRdrMgr(),
            counter_name='equation',
            base_use_doc_state_keys=['cnt-section'],
            numprefix_and_value_for_doc_state=None,
        )
        r = repr(dcs)
        self.assertTrue('cnt-section' in r)
        self.assertTrue('equation' in r)


# ----------------------------------------------------------------
# Integration: equations numbered within sections
# ----------------------------------------------------------------

class TestNumberingIntegration(unittest.TestCase):

    maxDiff = None

    def test_equations_numbered_within_sections(self):
        environ = mk_flm_environ_with_numbering(
            number_within={
                'equation': {
                    'reset_at': 'section',
                    'numprefix': '${section}.',
                }
            },
            headings_depth=True,
        )
        src = (
            r'\section{First}' '\n'
            r'\begin{equation}\label{eq:a}' '\n'
            r'E=mc^2' '\n'
            r'\end{equation}' '\n'
            r'\begin{equation}\label{eq:b}' '\n'
            r'F=ma' '\n'
            r'\end{equation}' '\n'
            r'\section{Second}' '\n'
            r'\begin{equation}\label{eq:c}' '\n'
            r'x^2+y^2=z^2' '\n'
            r'\end{equation}'
        )
        frag = environ.make_fragment(src.strip(), is_block_level=True)
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        # Section 1 equations: (1.1) and (1.2)
        self.assertTrue('\\tag*{(1.1)}' in result)
        self.assertTrue('\\tag*{(1.2)}' in result)
        # Section 2 equation: (2.1)
        self.assertTrue('\\tag*{(2.1)}' in result)

    def test_equations_without_numbering_feature(self):
        """Without numbering feature, equations get simple sequential numbers."""
        features = standard_features()
        environ = make_standard_environment(features)
        src = (
            r'\section{First}' '\n'
            r'\begin{equation}\label{eq:a}' '\n'
            r'E=mc^2' '\n'
            r'\end{equation}' '\n'
            r'\section{Second}' '\n'
            r'\begin{equation}\label{eq:b}' '\n'
            r'F=ma' '\n'
            r'\end{equation}'
        )
        frag = environ.make_fragment(src.strip(), is_block_level=True)
        doc = environ.make_document(frag.render)
        fr = HtmlFragmentRenderer()
        result, _ = doc.render(fr)
        # Sequential numbering: (1) and (2)
        self.assertTrue('\\tag*{(1)}' in result)
        self.assertTrue('\\tag*{(2)}' in result)


if __name__ == '__main__':
    import logging
    logging.basicConfig(level=logging.DEBUG)
    unittest.main()
