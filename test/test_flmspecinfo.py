import unittest

from flm import flmspecinfo 


class TestFLMSpecInfo(unittest.TestCase):

    def test_no_delayed_render_by_default(self):
        macrospecinfo = flmspecinfo.FLMSpecInfo(spec_node_parser_type='macro')
        self.assertFalse(macrospecinfo.delayed_render)





if __name__ == '__main__':
    unittest.main()
