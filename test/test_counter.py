import unittest

from flm import counter
from flm.counter import (
    alphacounter, Alphacounter,
    romancounter, Romancounter,
    fnsymbolcounter,
    customdigitscounter,
    unicodesuperscriptcounter, unicodesubscriptcounter,
    parse_counter_format_num, parse_counter_format_subnum,
    build_counter_formatter,
    CounterFormatter, ValueWithSubNums,
)


# this is just much shorter & easier...
V = counter.ValueWithSubNums


class TestAlphaCounter(unittest.TestCase):

    def test_lowercase_basic(self):
        self.assertEqual(alphacounter(1), 'a')
        self.assertEqual(alphacounter(2), 'b')
        self.assertEqual(alphacounter(26), 'z')

    def test_lowercase_wraps(self):
        self.assertEqual(alphacounter(27), 'aa')
        self.assertEqual(alphacounter(28), 'bb')
        self.assertEqual(alphacounter(52), 'zz')
        self.assertEqual(alphacounter(53), 'aaa')

    def test_uppercase(self):
        self.assertEqual(alphacounter(1, lower=False), 'A')
        self.assertEqual(alphacounter(26, lower=False), 'Z')
        self.assertEqual(alphacounter(27, lower=False), 'AA')

    def test_Alphacounter_shorthand(self):
        self.assertEqual(Alphacounter(1), 'A')
        self.assertEqual(Alphacounter(26), 'Z')
        self.assertEqual(Alphacounter(27), 'AA')


class TestRomanCounter(unittest.TestCase):

    def test_basic_values(self):
        self.assertEqual(romancounter(1), 'i')
        self.assertEqual(romancounter(4), 'iv')
        self.assertEqual(romancounter(9), 'ix')
        self.assertEqual(romancounter(49), 'xlix')
        self.assertEqual(romancounter(2099), 'mmxcix')

    def test_zero(self):
        self.assertEqual(romancounter(0), '')
        self.assertEqual(romancounter(0, zero='nulla'), 'nulla')

    def test_uppercase(self):
        self.assertEqual(romancounter(4, lower=False), 'IV')
        self.assertEqual(romancounter(2099, lower=False), 'MMXCIX')

    def test_Romancounter_shorthand(self):
        self.assertEqual(Romancounter(4), 'IV')
        self.assertEqual(Romancounter(2099), 'MMXCIX')


class TestFnSymbolCounter(unittest.TestCase):

    def test_first_cycle(self):
        self.assertEqual(fnsymbolcounter(1), '*')
        self.assertEqual(fnsymbolcounter(2), '\u2020')  # †
        self.assertEqual(fnsymbolcounter(3), '\u2021')  # ‡
        self.assertEqual(fnsymbolcounter(4), '\u00a7')  # §
        self.assertEqual(fnsymbolcounter(5), '\u00b6')  # ¶
        self.assertEqual(fnsymbolcounter(6), '\u2016')  # ‖

    def test_second_cycle(self):
        self.assertEqual(fnsymbolcounter(7), '**')
        self.assertEqual(fnsymbolcounter(12), '\u2016\u2016')

    def test_third_cycle(self):
        self.assertEqual(fnsymbolcounter(13), '***')

    def test_custom_symbols(self):
        self.assertEqual(fnsymbolcounter(1, symbols=['X', 'Y']), 'X')
        self.assertEqual(fnsymbolcounter(2, symbols=['X', 'Y']), 'Y')
        self.assertEqual(fnsymbolcounter(3, symbols=['X', 'Y']), 'XX')


class TestCustomDigitsCounter(unittest.TestCase):

    def test_zero(self):
        self.assertEqual(customdigitscounter(0), '')

    def test_decimal(self):
        self.assertEqual(customdigitscounter(1), '1')
        self.assertEqual(customdigitscounter(10), '10')
        self.assertEqual(customdigitscounter(255), '255')

    def test_binary_custom(self):
        self.assertEqual(customdigitscounter(5, 'FT'), 'TFT')
        self.assertEqual(customdigitscounter(0, 'FT'), '')
        self.assertEqual(customdigitscounter(1, 'FT'), 'T')
        self.assertEqual(customdigitscounter(2, 'FT'), 'TF')


class TestUnicodeCounters(unittest.TestCase):

    def test_superscript(self):
        self.assertEqual(unicodesuperscriptcounter(17), '\u00b9\u2077')
        self.assertEqual(unicodesuperscriptcounter(0), '')

    def test_subscript(self):
        self.assertEqual(unicodesubscriptcounter(17), '\u2081\u2087')
        self.assertEqual(unicodesubscriptcounter(0), '')


class TestValueWithSubNums(unittest.TestCase):

    def test_construct_1(self):
        v = V(1)
        self.assertEqual(len(v.values_tuple), 1)
        self.assertEqual(v.values_tuple, (1,) )

    def test_construct_2(self):
        v = V(1, (2,3,))
        self.assertEqual(len(v.values_tuple), 3)
        self.assertEqual(v.values_tuple, (1,2,3,) )


    def test_astuple(self):
        self.assertEqual(V(1).astuple(), (1,))
        self.assertEqual(V(1, ()).astuple(), (1,))
        self.assertEqual(V(1, (0,)).astuple(), (1,0,))
        self.assertEqual(V(1, (2,3,)).astuple(), (1,2,3))

    def test_targetidstr(self):
        self.assertEqual(V(1).targetidstr(), "1")
        self.assertEqual(V(1, ()).targetidstr(), "1")
        self.assertEqual(V(1, (0,)).targetidstr(), "1-0")
        self.assertEqual(V(1, (2,3,)).targetidstr(), "1-2-3")


    def test_does_immediately_succeed(self):
        self.assertTrue(
            V(1).does_immediately_succeed(V(0))
        )
        self.assertTrue(
            V(124).does_immediately_succeed(V(123))
        )
        self.assertFalse(
            V(1).does_immediately_succeed(V(1))
        )
        self.assertFalse(
            V(3).does_immediately_succeed(V(1))
        )
        self.assertFalse(
            V(2, (1,)).does_immediately_succeed(V(1))
        )
        self.assertFalse(
            V(2).does_immediately_succeed(V(1,(9,)))
        )
        self.assertTrue(
            V(2, (3,4,)).does_immediately_succeed(V(2, (3,3,)))
        )
        self.assertFalse(
            V(1, (1,)).does_immediately_succeed(V(1, (1,)))
        )
        self.assertFalse(
            V(1, (1,2,1)).does_immediately_succeed(V(1, (1,2)))
        )



    def test_get_num(self):
        self.assertEqual(V(3, (2, 1)).get_num(), 3)
        self.assertEqual(V(7).get_num(), 7)

    def test_get_subnums(self):
        self.assertEqual(V(3, (2, 1)).get_subnums(), (2, 1))
        self.assertEqual(V(7).get_subnums(), ())

    def test_equals(self):
        self.assertTrue(V(3, (2, 1)).equals(V(3, (2, 1))))
        self.assertFalse(V(3, (2, 1)).equals(V(3, (2, 2))))
        self.assertFalse(V(3).equals(V(4)))

    def test_incremented_default(self):
        self.assertEqual(V(3, (2, 1)).incremented().values_tuple, (3, 2, 2))

    def test_incremented_level0(self):
        self.assertEqual(V(3, (2, 1)).incremented(subnum_level=0).values_tuple, (4, 0, 0))

    def test_incremented_level1(self):
        self.assertEqual(V(3, (2, 1)).incremented(subnum_level=1).values_tuple, (3, 3, 0))

    def test_extended_by_one_default(self):
        self.assertEqual(V(3).extended_by_one().values_tuple, (3, 0))

    def test_extended_by_one_with_value(self):
        self.assertEqual(V(3).extended_by_one(subnum_value=5).values_tuple, (3, 5))

    def test_repr(self):
        self.assertEqual(repr(V(3, (2, 1))), 'V{(3, 2, 1)}')

    def test_copy_from_valuewithsubnums(self):
        v1 = V(1, (2, 3))
        v2 = V(v1)
        self.assertEqual(v2.values_tuple, (1, 2, 3))

    def test_targetidstr_with_numprefix(self):
        self.assertEqual(V(1, (2,)).targetidstr(numprefix='sec:'), 'sec-1-2')

    def test_invalid_value_raises(self):
        with self.assertRaises(ValueError):
            V('not_an_int')


class TestStandardCounterFormatters(unittest.TestCase):

    def test_roman(self):
        roman = counter.standard_counter_formatters['roman']
        self.assertEqual(roman(1), 'i')
        self.assertEqual(roman(2), 'ii')
        self.assertEqual(roman(4), 'iv')
        self.assertEqual(roman(5), 'v')
        self.assertEqual(roman(9), 'ix')
        self.assertEqual(roman(49), 'xlix')
        self.assertEqual(roman(2099), 'mmxcix')

    def test_Roman(self):
        Roman = counter.standard_counter_formatters['Roman']
        self.assertEqual(Roman(1), 'I')
        self.assertEqual(Roman(2), 'II')
        self.assertEqual(Roman(4), 'IV')
        self.assertEqual(Roman(5), 'V')
        self.assertEqual(Roman(9), 'IX')
        self.assertEqual(Roman(49), 'XLIX')
        self.assertEqual(Roman(2099), 'MMXCIX')

    def test_alph(self):
        alph = counter.standard_counter_formatters['alph']
        self.assertEqual(alph(1), 'a')
        self.assertEqual(alph(16), 'p')
        self.assertEqual(alph(26), 'z')
        self.assertEqual(alph(27), 'aa')
        self.assertEqual(alph(28), 'bb')

    def test_Alph(self):
        Alph = counter.standard_counter_formatters['Alph']
        self.assertEqual(Alph(1), 'A')
        self.assertEqual(Alph(16), 'P')
        self.assertEqual(Alph(26), 'Z')
        self.assertEqual(Alph(27), 'AA')
        self.assertEqual(Alph(28), 'BB')


    def test_arabic(self):
        self.assertEqual(
            counter.standard_counter_formatters['arabic'](1),
            '1'
        )

        self.assertEqual(
            counter.standard_counter_formatters['arabic'](124),
            '124'
        )

        self.assertEqual(
            counter.standard_counter_formatters['arabic'](0),
            '0'
        )


class TestCounterFormatter(unittest.TestCase):

    def test_simple(self):
        
        f = counter.CounterFormatter(lambda n, numprefix=None: f"({numprefix or ''}{n})",
                                     "Eq.~",
                                     delimiters=('[', ']'),
                                     join_spec='compact')
        
        self.assertEqual(
            f.format_flm(1),
            "Eq.~[(1)]"
        )
        self.assertEqual(
            f.format_flm(-992),
            "Eq.~[(-992)]"
        )
        self.assertEqual(
            f.format_flm(V(1)),
            "Eq.~[(1)]"
        )
        self.assertEqual(
            f.format_flm(V(-992)),
            "Eq.~[(-992)]"
        )
        self.assertEqual(
            f.format_many_flm([1]),
            "Eq.~[(1)]"
        )
        self.assertEqual(
            f.format_many_flm([2,3,1]),
            "Eq.~[(1)–(3)]"
        )
        self.assertEqual(
            f.format_many_flm([8,9,10,11]),
            "Eq.~[(8)–(11)]"
        )
        self.assertEqual(
            f.format_many_flm([2,1,99,3]),
            "Eq.~[(1)–(3),(99)]"
        )
        self.assertEqual(
            f.format_many_flm([1,3,99,2,98,54]),
            "Eq.~[(1)–(3),(54),(98),(99)]"
        )

    def test_join(self):
        
        jspec = {
            'one_pre': '<',
            'one_post': '>',
            'pair_pre': '((',
            'pair_mid': '||',
            'pair_post': '))',
            'range_pre': '[[',
            'range_mid': '--',
            'range_pairmid': '|',
            'range_post': ']]',
            'list_pre': '<<',
            'list_mid': ';',
            'list_midlast': ';&',
            'list_post': '>>',
        }

        f = counter.CounterFormatter(
            {'template': "${Roman}"},
            prefix_display={
                'singular': "eq.~",
                'plural': "eqs.~",
                'capital': {
                    'singular': "Equation~",
                    'plural': "Equations~",
                    4: "Quartet of Equations~",
                },
            },
            delimiters=('!<! ', ' !>!'),
            join_spec=jspec,
            name_in_link=True
        )
        
        self.assertEqual(
            f.format_flm(1),
            "eq.~!<! I !>!"
        )
        self.assertEqual(
            f.format_flm(V(1)),
            "eq.~!<! I !>!"
        )
        self.assertEqual(
            f.format_many_flm([1]),
            "eq.~!<! <I> !>!"
        )
        self.assertEqual(
            f.format_many_flm([2,1]),
            "eqs.~!<! ((I||II)) !>!"
        )
        self.assertEqual(
            f.format_many_flm([2,3,1]),
            "eqs.~!<! <[[I--III]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm([2,1,99,3], prefix_variant='capital'),
            "Quartet of Equations~!<! (([[I--III]]||XCIX)) !>!"
        )
        self.assertEqual(
            f.format_many_flm([1,3,99,2,98,54], prefix_variant='capital'),
            "Equations~!<! <<[[I--III]];LIV;&[[XCVIII|XCIX]]>> !>!"
        )

        self.assertEqual(
            f.format_many_flm([2,3,1], prefix_variant='capital'),
            "Equations~!<! <[[I--III]]> !>!"
        )
        self.assertEqual(
            f.format_flm(1, prefix_variant='capital'),
            "Equation~!<! I !>!"
        )
        self.assertEqual(
            f.format_many_flm([1], prefix_variant='capital'),
            "Equation~!<! <I> !>!"
        )


        def wrap_link_fn(n, s, **kwargs):
            return r'\mylink{' + str(n) + '}{' + s + '}'

        self.assertEqual(
            f.format_flm(1, wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{eq.~!<! I !>!}"
        )
        self.assertEqual(
            f.format_many_flm([1], wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{eq.~!<! <I> !>!}"
        )
        self.assertEqual(
            f.format_many_flm([2,3], wrap_link_fn=wrap_link_fn),
            r"\mylink{2}{eqs.~}!<! ((\mylink{2}{II}||\mylink{3}{III})) !>!"
        )
        self.assertEqual(
            f.format_many_flm([2,3,1], wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{eqs.~}!<! <[[\mylink{1}{I}--\mylink{3}{III}]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm(
                [1,3,99,2,98,54], wrap_link_fn=wrap_link_fn,
                prefix_variant='capital'
            ),
            r"\mylink{1}{Equations~}!<! <<[[\mylink{1}{I}--\mylink{3}{III}]];\mylink{54}{LIV};&[[\mylink{98}{XCVIII}|\mylink{99}{XCIX}]]>> !>!"
        )


    def test_join_noinnerdelim(self):
        
        jspec = {
            'one_pre': '',
            'one_post': '',
            'pair_pre': '',
            'pair_mid': '|',
            'pair_post': '',
            'range_pre': '',
            'range_mid': '--',
            'range_pairmid': '|',
            'range_post': '',
            'list_pre': '<<',
            'list_mid': ';',
            'list_midlast': ';',
            'list_post': '>>',
        }

        f = counter.CounterFormatter(
            {'template': "${Alph}"},
            prefix_display={
                'singular': "",
                'plural': "",
            },
            delimiters=('[', ']'),
            join_spec=jspec,
            name_in_link=True
        )

        def wrap_link_fn(n, s, **kwargs):
            return r'\mylink{' + str(n) + '}{' + s + '}'

        self.assertEqual(
            f.format_many_flm([1], wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{[A]}"
        )

        self.assertEqual(
            f.format_many_flm([2,3], wrap_link_fn=wrap_link_fn),
            r"[\mylink{2}{B}|\mylink{3}{C}]"
        )

        self.assertEqual(
            f.format_many_flm([2,3,1], wrap_link_fn=wrap_link_fn),
            r"[\mylink{1}{A}--\mylink{3}{C}]"
        )

    def test_join_withnumprefix(self):
        
        jspec = {
            'one_pre': '<',
            'one_post': '>',
            'pair_pre': '((',
            'pair_mid': '||',
            'pair_post': '))',
            'range_pre': '[[',
            'range_mid': '--',
            'range_pairmid': '|',
            'range_post': ']]',
            'list_pre': '<<',
            'list_mid': ';',
            'list_midlast': ';&',
            'list_post': '>>',
        }

        f = counter.CounterFormatter(
            {'template': "${Roman}"},
            prefix_display={
                'singular': "eq.~",
                'plural': "eqs.~",
                'capital': {
                    'singular': "Equation~",
                    'plural': "Equations~",
                    4: "Quartet of Equations~",
                },
            },
            delimiters=('!<! ', ' !>!'),
            join_spec=jspec,
            name_in_link=True,
            #repeat_numprefix_in_range=False, #the default
        )
        
        self.assertEqual(
            f.format_many_flm( [('A-', [1])] ),
            "eq.~!<! <A-I> !>!"
        )
        self.assertEqual(
            f.format_flm( 1, numprefix='A-' ),
            "eq.~!<! A-I !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,1])] ),
            "eqs.~!<! ((A-I||A-II)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2]), ('B-', [1])] ),
            "eqs.~!<! ((A-II||B-I)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,3,1])] ),
            "eqs.~!<! <[[A.I--III]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,3,1]), ('B.', [1])] ),
            "eqs.~!<! (([[A.I--III]]||B.I)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,1,99,3])], prefix_variant='capital'),
            "Quartet of Equations~!<! (([[A.I--III]]||A.XCIX)) !>!"
        )
        self.assertEqual(
            f.format_many_flm([('A-', [1,3,99,98,2]),('C.', [54])],
                              prefix_variant='capital'),
            "Equations~!<! <<[[A-I--III]];[[A-XCVIII|A-XCIX]];&C.LIV>> !>!"
        )

        self.assertEqual(
            f.format_many_flm( [('A-', [2,3,1])], prefix_variant='capital'),
            "Equations~!<! <[[A-I--III]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [1])], prefix_variant='capital'),
            "Equation~!<! <A.I> !>!"
        )

        def wrap_link_fn(*, n, s, numprefix, **kwargs):
            return r'\mylink{p/' + (numprefix or '') + str(n) + '}{' + s + '}'

        self.assertEqual(
            f.format_flm( 1, numprefix='A-', wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eq.~!<! A-I !>!}"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [1])] , wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eq.~!<! <A-I> !>!}"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,3])], wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-2}{eqs.~}!<! ((\mylink{p/A-2}{A-II}||\mylink{p/A-3}{A-III})) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,3,1])], wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eqs.~}!<! <[[\mylink{p/A-1}{A-I}--\mylink{p/A-3}{III}]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,3,1]), ('B-', [1,3,2])], wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eqs.~}!<! (([[\mylink{p/A-1}{A-I}--\mylink{p/A-3}{III}]]||[[\mylink{p/B-1}{B-I}--\mylink{p/B-3}{III}]])) !>!"
        )
        self.assertEqual(
            f.format_many_flm(
                [('B-', [1,3,99,2,98]), ('A-',[54])],
                wrap_link_fn=wrap_link_fn,
                prefix_variant='capital'
            ),
            r"\mylink{p/B-1}{Equations~}!<! <<[[\mylink{p/B-1}{B-I}--\mylink{p/B-3}{III}]];[[\mylink{p/B-98}{B-XCVIII}|\mylink{p/B-99}{B-XCIX}]];&\mylink{p/A-54}{A-LIV}>> !>!"
        )

    def test_join_withnumprefix_2(self):
        
        jspec = {
            'one_pre': '<',
            'one_post': '>',
            'pair_pre': '((',
            'pair_mid': '||',
            'pair_post': '))',
            'range_pre': '[[',
            'range_mid': '--',
            'range_pairmid': '|',
            'range_post': ']]',
            'list_pre': '<<',
            'list_mid': ';',
            'list_midlast': ';&',
            'list_post': '>>',
        }

        f = counter.CounterFormatter(
            {'template': "${Roman}"},
            prefix_display={
                'singular': "eq.~",
                'plural': "eqs.~",
                'capital': {
                    'singular': "Equation~",
                    'plural': "Equations~",
                    4: "Quartet of Equations~",
                },
            },
            delimiters=('!<! ', ' !>!'),
            join_spec=jspec,
            name_in_link=True,
            repeat_numprefix_in_range=True,
        )
        
        self.assertEqual(
            f.format_many_flm( [('A-', [1])] ),
            "eq.~!<! <A-I> !>!"
        )
        self.assertEqual(
            f.format_flm( 1, numprefix='A-' ),
            "eq.~!<! A-I !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,1])] ),
            "eqs.~!<! ((A-I||A-II)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2]), ('B-', [1])] ),
            "eqs.~!<! ((A-II||B-I)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,3,1])] ),
            "eqs.~!<! <[[A.I--A.III]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,3,1]), ('B.', [1])] ),
            "eqs.~!<! (([[A.I--A.III]]||B.I)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,1,99,3])], prefix_variant='capital'),
            "Quartet of Equations~!<! (([[A.I--A.III]]||A.XCIX)) !>!"
        )
        self.assertEqual(
            f.format_many_flm([('A-', [1,3,99,98,2]),('C.', [54])],
                              prefix_variant='capital'),
            "Equations~!<! <<[[A-I--A-III]];[[A-XCVIII|A-XCIX]];&C.LIV>> !>!"
        )

        self.assertEqual(
            f.format_many_flm( [('A-', [2,3,1])], prefix_variant='capital'),
            "Equations~!<! <[[A-I--A-III]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [1])], prefix_variant='capital'),
            "Equation~!<! <A.I> !>!"
        )

        def wrap_link_fn(*, n, s, numprefix, **kwargs):
            return r'\mylink{p/' + (numprefix or '') + str(n) + '}{' + s + '}'

        self.assertEqual(
            f.format_flm( 1, numprefix='A-', wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eq.~!<! A-I !>!}"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [1])] , wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eq.~!<! <A-I> !>!}"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,3])], wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-2}{eqs.~}!<! ((\mylink{p/A-2}{A-II}||\mylink{p/A-3}{A-III})) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,3,1])], wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eqs.~}!<! <[[\mylink{p/A-1}{A-I}--\mylink{p/A-3}{A-III}]]> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [2,3,1]), ('B-', [1,3,2])], wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eqs.~}!<! (([[\mylink{p/A-1}{A-I}--\mylink{p/A-3}{A-III}]]||[[\mylink{p/B-1}{B-I}--\mylink{p/B-3}{B-III}]])) !>!"
        )
        self.assertEqual(
            f.format_many_flm(
                [('B-', [1,3,99,2,98]), ('A-',[54])],
                wrap_link_fn=wrap_link_fn,
                prefix_variant='capital'
            ),
            r"\mylink{p/B-1}{Equations~}!<! <<[[\mylink{p/B-1}{B-I}--\mylink{p/B-3}{B-III}]];[[\mylink{p/B-98}{B-XCVIII}|\mylink{p/B-99}{B-XCIX}]];&\mylink{p/A-54}{A-LIV}>> !>!"
        )




    def test_join_withsubnums(self):
        
        jspec = {
            'one_pre': '<',
            'one_post': '>',
            'pair_pre': '((',
            'pair_mid': '||',
            'pair_post': '))',
            'range_pre': '[[',
            'range_mid': '--',
            'range_pairmid': '|',
            'range_post': ']]',
            'list_pre': '<<',
            'list_mid': ';',
            'list_midlast': ';&',
            'list_post': '>>',
        }

        f = counter.CounterFormatter(
            {'template': "${Roman}"},
            prefix_display={
                'singular': "eq.~",
                'plural': "eqs.~",
                'capital': {
                    'singular': "Equation~",
                    'plural': "Equations~",
                    4: "Quartet of Equations~",
                },
            },
            delimiters=('!<! ', ' !>!'),
            join_spec=jspec,
            name_in_link=True,
            subnums_format_nums=(
                {'format_num': {'template': "${alph}"}, 'prefix': '.'},
                {'format_num': {'template': "${roman}"}, 'prefix': '.'},
            ),
        )

        self.assertEqual(
            f.format_many_flm( [('A-', [V(1,(2,3))])] ),
            "eq.~!<! <A-I.b.iii> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [V(1,(2,))])] ),
            "eq.~!<! <A-I.b> !>!"
        )
        self.assertEqual(
            f.format_flm( 1, numprefix='A-', subnums=(2,3,) ),
            "eq.~!<! A-I.b.iii !>!"
        )
        self.assertEqual(
            f.format_flm( 1, numprefix='A-', subnums=(3,) ),
            "eq.~!<! A-I.c !>!"
        )
        self.assertEqual(
            f.format_flm( V(1, (2,3)), numprefix='A-' ),
            "eq.~!<! A-I.b.iii !>!"
        )
        self.assertEqual(
            f.format_flm( V(1, (3,)), numprefix='A-' ),
            "eq.~!<! A-I.c !>!"
        )
        with self.assertRaises(ValueError):
            f.format_flm( V(1, (3,)), numprefix='A-', subnums=(3,) ),

        self.assertEqual(
            f.format_many_flm( [('A-', [V(2,(3,)),V(1,(3,1))])] ),
            "eqs.~!<! ((A-I.c.i||A-II.c)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [V(2,)]), ('B-', [V(1,)])] ),
            "eqs.~!<! ((A-II||B-I)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [V(2,)]), ('B-', [V(1,),V(1,(1,))])] ),
            "eqs.~!<! <<A-II;B-I;&B-I.a>> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [2,V(3,(1,)),1])] ),
            "eqs.~!<! (([[A.I|A.II]]||A.III.a)) !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [V(2,(2,)),V(2,(3,)),1]), ('B.', [1])] ),
            "eqs.~!<! <<A.I;[[A.II.b|A.II.c]];&B.I>> !>!"
        )
        self.assertEqual(
            f.format_many_flm( [('A.', [V(2,(2,)),V(2,(5,)),V(2,(4,)),V(2,(3,)),1]), ('B.', [1])] ),
            "eqs.~!<! <<A.I;[[A.II.b--e]];&B.I>> !>!"
        )

        def wrap_link_fn(*, n, s, numprefix, subnums):
            if subnums is None:
                subnums = ()
            return r'\mylink{p/' + (numprefix or '') + str(n) + ''.join([f'.{s}' for s in subnums]) + '}{' + s + '}'

        self.assertEqual(
            f.format_flm( 1, numprefix='A-', wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1}{eq.~!<! A-I !>!}"
        )
        self.assertEqual(
            f.format_flm( 1, subnums=(2,1,), numprefix='A-', wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1.2.1}{eq.~!<! A-I.b.i !>!}"
        )
        self.assertEqual(
            f.format_many_flm( [('A-', [V(1,(1,))])] , wrap_link_fn=wrap_link_fn),
            r"\mylink{p/A-1.1}{eq.~!<! <A-I.a> !>!}"
        )
        self.assertEqual(
            f.format_many_flm(
                [('B-', [1,3,V(99,(3,)),2,V(99,(2,))]), ('A-',[54])],
                wrap_link_fn=wrap_link_fn,
                prefix_variant='capital'
            ),
            r"\mylink{p/B-1}{Equations~}!<! <<[[\mylink{p/B-1}{B-I}--\mylink{p/B-3}{III}]];[[\mylink{p/B-99.2}{B-XCIX.b}|\mylink{p/B-99.3}{B-XCIX.c}]];&\mylink{p/A-54}{A-LIV}>> !>!"
        )




class TestParseCounterFormatNum(unittest.TestCase):

    def test_named_formatter(self):
        fn = parse_counter_format_num('arabic')
        self.assertEqual(fn(5), '5')
        self.assertEqual(fn(5, numprefix='X-'), 'X-5')

    def test_template_dict(self):
        fn = parse_counter_format_num({'template': '${arabic}.'})
        self.assertEqual(fn(5), '5.')

    def test_callable_passthrough(self):
        fn = parse_counter_format_num(lambda n, numprefix=None: '#' + str(n))
        self.assertEqual(fn(3), '#3')

    def test_tag_template(self):
        fn = parse_counter_format_num('(a)', str_use_tag_template=True)
        self.assertEqual(fn(1), '(a)')
        self.assertEqual(fn(3), '(c)')

    def test_tag_template_no_counter(self):
        fn = parse_counter_format_num('*', str_use_tag_template=True)
        self.assertEqual(fn(1), '*')
        self.assertEqual(fn(99), '*')

    def test_invalid_int_raises(self):
        with self.assertRaises(ValueError):
            parse_counter_format_num(12345)

    def test_invalid_string_raises(self):
        with self.assertRaises(ValueError):
            parse_counter_format_num('nonexistent')


class TestParseCounterFormatSubnum(unittest.TestCase):

    def test_dict_with_format_num_and_prefix(self):
        sfn = parse_counter_format_subnum({'format_num': 'alph', 'prefix': '.'})
        res = sfn(2)
        self.assertEqual(res['formatted'], 'b')
        self.assertEqual(res['prefix'], '.')

    def test_callable_passthrough(self):
        fn = lambda n: {'formatted': str(n), 'prefix': '-'}
        sfn = parse_counter_format_subnum(fn)
        self.assertIs(sfn, fn)


class TestBuildCounterFormatter(unittest.TestCase):

    def test_string_spec(self):
        cf = build_counter_formatter(
            'arabic',
            {'format_num': 'roman', 'prefix_display': 'Eq.'},
            counter_formatter_id='test'
        )
        self.assertEqual(cf.counter_formatter_id, 'test')
        self.assertEqual(cf.format_num, 'arabic')

    def test_none_uses_default(self):
        cf = build_counter_formatter(
            None,
            {'format_num': 'roman'},
            counter_formatter_id='test2'
        )
        self.assertEqual(cf.format_num, 'roman')
        self.assertEqual(cf.counter_formatter_id, 'test2')

    def test_template_dict(self):
        cf = build_counter_formatter(
            {'template': '${arabic}.'},
            {'format_num': 'roman'},
            counter_formatter_id='t3'
        )
        self.assertEqual(cf.format_num, {'template': '${arabic}.'})

    def test_dict_without_template(self):
        cf = build_counter_formatter(
            {'format_num': 'alph', 'prefix_display': 'Item '},
            {'format_num': 'roman'},
            counter_formatter_id='t4'
        )
        self.assertEqual(cf.format_num, 'alph')
        self.assertEqual(cf.prefix_display, {'singular': 'Item ', 'plural': 'Item '})

    def test_existing_counter_formatter_keeps_id(self):
        cf = CounterFormatter('arabic', counter_formatter_id='original')
        result = build_counter_formatter(cf, {'format_num': 'roman'}, counter_formatter_id='new')
        self.assertEqual(result.counter_formatter_id, 'original')

    def test_existing_counter_formatter_sets_id_if_none(self):
        cf = CounterFormatter('arabic')
        result = build_counter_formatter(cf, {'format_num': 'roman'}, counter_formatter_id='t6')
        self.assertEqual(result.counter_formatter_id, 't6')

    def test_invalid_spec_raises(self):
        with self.assertRaises(ValueError):
            build_counter_formatter([1, 2, 3], {'format_num': 'roman'}, counter_formatter_id='x')


class TestCounterFormatterExtras(unittest.TestCase):

    maxDiff = None

    def test_format_number(self):
        cf = CounterFormatter('arabic', prefix_display='Eq.~', delimiters=('(', ')'))
        self.assertEqual(cf.format_number(5), '5')
        self.assertEqual(cf.format_number(5, numprefix='X-'), 'X-5')

    def test_asdict(self):
        cf = CounterFormatter('arabic', prefix_display='Eq.~', delimiters=('(', ')'))
        d = cf.asdict()
        self.assertEqual(d['format_num'], 'arabic')
        self.assertEqual(d['prefix_display'], {'singular': 'Eq.~', 'plural': 'Eq.~'})
        self.assertEqual(d['delimiters'], ('(', ')'))
        self.assertTrue('join_spec' in d)
        self.assertTrue('name_in_link' in d)

    def test_format_many_empty(self):
        cf = CounterFormatter('arabic')
        self.assertEqual(cf.format_many_flm([]), '(empty)')

    def test_default_prefix_display(self):
        cf = CounterFormatter('arabic')
        self.assertEqual(cf.prefix_display, {'singular': '', 'plural': ''})

    def test_default_delimiters(self):
        cf = CounterFormatter('arabic')
        self.assertEqual(cf.delimiters, ('', ''))

    def test_format_flm_without_delimiters(self):
        cf = CounterFormatter('arabic', delimiters=('(', ')'))
        self.assertEqual(cf.format_flm(5, with_delimiters=False), '5')

    def test_format_flm_without_prefix(self):
        cf = CounterFormatter('arabic', prefix_display='Eq. ')
        self.assertEqual(cf.format_flm(5, with_prefix=False), '5')
        self.assertEqual(cf.format_flm(5, with_prefix=True), 'Eq. 5')


if __name__ == '__main__':
    unittest.main()
