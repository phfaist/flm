import unittest

from llm import fmthelpers

class TestStandardCounterFormatters(unittest.TestCase):
    
    def test_roman(self):
        roman = fmthelpers.standard_counter_formatters['roman']
        self.assertEqual(roman(1), 'i')
        self.assertEqual(roman(2), 'ii')
        self.assertEqual(roman(4), 'iv')
        self.assertEqual(roman(5), 'v')
        self.assertEqual(roman(9), 'ix')
        self.assertEqual(roman(49), 'xlix')
        self.assertEqual(roman(2099), 'mmxcix')

    def test_Roman(self):
        Roman = fmthelpers.standard_counter_formatters['Roman']
        self.assertEqual(Roman(1), 'I')
        self.assertEqual(Roman(2), 'II')
        self.assertEqual(Roman(4), 'IV')
        self.assertEqual(Roman(5), 'V')
        self.assertEqual(Roman(9), 'IX')
        self.assertEqual(Roman(49), 'XLIX')
        self.assertEqual(Roman(2099), 'MMXCIX')

    def test_alph(self):
        alph = fmthelpers.standard_counter_formatters['alph']
        self.assertEqual(alph(1), 'a')
        self.assertEqual(alph(16), 'p')
        self.assertEqual(alph(26), 'z')
        self.assertEqual(alph(27), 'aa')
        self.assertEqual(alph(28), 'bb')

    def test_Alph(self):
        Alph = fmthelpers.standard_counter_formatters['Alph']
        self.assertEqual(Alph(1), 'A')
        self.assertEqual(Alph(16), 'P')
        self.assertEqual(Alph(26), 'Z')
        self.assertEqual(Alph(27), 'AA')
        self.assertEqual(Alph(28), 'BB')


    def test_arabic(self):
        self.assertEqual(
            fmthelpers.standard_counter_formatters['arabic'](1),
            '1'
        )

        self.assertEqual(
            fmthelpers.standard_counter_formatters['arabic'](124),
            '124'
        )

        self.assertEqual(
            fmthelpers.standard_counter_formatters['arabic'](0),
            '0'
        )



if __name__ == '__main__':
    unittest.main()
