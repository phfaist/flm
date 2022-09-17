import unittest

from llm.fragmentrenderer.html import HtmlFragmentRenderer



class TestHtmlFragmentRenderer(unittest.TestCase):

    def test_escape_attribs(self):
        fr = HtmlFragmentRenderer()

        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1&b=2'),
            'https://example.com/page?a=1&b=2'
        )

        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1"&b=2'),
            'https://example.com/page?a=1&quot;&b=2'
        )

        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1&b;2'),
            'https://example.com/page?a=1&amp;b;2'
        )

        fr.aggressively_escape_html_attributes = True
        self.assertEqual(
            fr.htmlescape_double_quoted_attribute_value('https://example.com/page?a=1&b=2'),
            'https://example.com/page?a=1&amp;b=2'
        )




if __name__ == '__main__':
    unittest.main()
