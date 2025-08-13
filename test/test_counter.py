import unittest

from flm import counter

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

        V = counter.ValueWithSubNums
        
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





if __name__ == '__main__':
    unittest.main()
