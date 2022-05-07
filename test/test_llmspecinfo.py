import unittest

from llm.llmspecinfo import (
    LLMSpecInfo,
    LLMSpecInfoSpecClass,
    LLMMacroSpec,
    LLMEnvironmentSpec,
    LLMSpecialsSpec,
    TextFormat,
    Verbatim,
    MathEnvironment,
    Error
)


class TestLLMSpecInfo(unittest.TestCase):
    def test_no_delayed_render_by_default(self):
        self.assertFalse(LLMSpecInfo().delayed_render)

class TestLLMSpecInfoSpecClass(unittest.TestCase):

    def test_init_value_instance(self):

        specinfo = LLMSpecInfo()
        sisc = LLMSpecInfoSpecClass(specinfo)
        self.assertIs(sisc.llm_specinfo, specinfo)
        self.assertIsNone(sisc.llm_specinfo_string)

    def test_init_value_string(self):

        sisc = LLMSpecInfoSpecClass('X')
        self.assertEqual(sisc.llm_specinfo, 'X')
        self.assertEqual(sisc.llm_specinfo_string, 'X')

    def test_finalize_node_sets_specinfo(self):

        class _Store:
            pass
        dummynode = _Store()

        specinfo = LLMSpecInfo()
        sisc = LLMSpecInfoSpecClass(specinfo)
        sisc.finalize_node(dummynode)

        self.assertIs(dummynode.llm_specinfo, specinfo)


if __name__ == '__main__':
    unittest.main()
