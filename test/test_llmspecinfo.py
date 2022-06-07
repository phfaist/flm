import unittest

from llm import llmspecinfo 


class TestLLMSpecInfo(unittest.TestCase):
    def test_no_delayed_render_by_default(self):
        self.assertFalse(llmspecinfo.LLMSpecInfo().delayed_render)





if __name__ == '__main__':
    unittest.main()
