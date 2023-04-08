import unittest

from llm import counter

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
        
        f = counter.CounterFormatter(lambda n: f"({n})",
                                     "Eq.~",
                                     delimiters=('[', ']'),
                                     join_spec='compact')
        
        self.assertEqual(
            f.format_llm(1),
            "Eq.~[(1)]"
        )
        self.assertEqual(
            f.format_llm(-992),
            "Eq.~[(-992)]"
        )
        self.assertEqual(
            f.format_many_llm([1]),
            "Eq.~[(1)]"
        )
        self.assertEqual(
            f.format_many_llm([2,3,1]),
            "Eq.~[(1)–(3)]"
        )
        self.assertEqual(
            f.format_many_llm([8,9,10,11]),
            "Eq.~[(8)–(11)]"
        )
        self.assertEqual(
            f.format_many_llm([2,1,99,3]),
            "Eq.~[(1)–(3),(99)]"
        )
        self.assertEqual(
            f.format_many_llm([1,3,99,2,98,54]),
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
            f.format_llm(1),
            "eq.~!<! I !>!"
        )
        self.assertEqual(
            f.format_many_llm([1]),
            "eq.~!<! <I> !>!"
        )
        self.assertEqual(
            f.format_many_llm([2,1]),
            "eqs.~!<! ((I||II)) !>!"
        )
        self.assertEqual(
            f.format_many_llm([2,3,1]),
            "eqs.~!<! <[[I--III]]> !>!"
        )
        self.assertEqual(
            f.format_many_llm([2,1,99,3], prefix_variant='capital'),
            "Quartet of Equations~!<! (([[I--III]]||XCIX)) !>!"
        )
        self.assertEqual(
            f.format_many_llm([1,3,99,2,98,54], prefix_variant='capital'),
            "Equations~!<! <<[[I--III]];LIV;&[[XCVIII|XCIX]]>> !>!"
        )

        self.assertEqual(
            f.format_many_llm([2,3,1], prefix_variant='capital'),
            "Equations~!<! <[[I--III]]> !>!"
        )
        self.assertEqual(
            f.format_llm(1, prefix_variant='capital'),
            "Equation~!<! I !>!"
        )
        self.assertEqual(
            f.format_many_llm([1], prefix_variant='capital'),
            "Equation~!<! <I> !>!"
        )


        def wrap_link_fn(n, s):
            return r'\mylink{' + str(n) + '}{' + s + '}'

        self.assertEqual(
            f.format_llm(1, wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{eq.~!<! I !>!}"
        )
        self.assertEqual(
            f.format_many_llm([1], wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{eq.~!<! <I> !>!}"
        )
        self.assertEqual(
            f.format_many_llm([2,3], wrap_link_fn=wrap_link_fn),
            r"\mylink{2}{eqs.~}!<! ((\mylink{2}{II}||\mylink{3}{III})) !>!"
        )
        self.assertEqual(
            f.format_many_llm([2,3,1], wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{eqs.~}!<! <[[\mylink{1}{I}--\mylink{3}{III}]]> !>!"
        )
        self.assertEqual(
            f.format_many_llm(
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

        def wrap_link_fn(n, s):
            return r'\mylink{' + str(n) + '}{' + s + '}'

        self.assertEqual(
            f.format_many_llm([1], wrap_link_fn=wrap_link_fn),
            r"\mylink{1}{[A]}"
        )

        self.assertEqual(
            f.format_many_llm([2,3], wrap_link_fn=wrap_link_fn),
            r"[\mylink{2}{B}|\mylink{3}{C}]"
        )

        self.assertEqual(
            f.format_many_llm([2,3,1], wrap_link_fn=wrap_link_fn),
            r"[\mylink{1}{A}--\mylink{3}{C}]"
        )




if __name__ == '__main__':
    unittest.main()
