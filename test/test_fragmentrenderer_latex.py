import unittest

from flm.fragmentrenderer.latex import LatexFragmentRenderer



class TestLatexFragmentRenderer(unittest.TestCase):

    def test_esc_00(self):
        lfr = LatexFragmentRenderer()

        self.assertEqual(
            lfr.latexescape('{x}'),
            r'\{x\}'
        )




if __name__ == '__main__':
    unittest.main()
