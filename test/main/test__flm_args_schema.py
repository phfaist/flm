import unittest


import typing
from typing import Optional, TypedDict
from collections.abc import Sequence, Mapping

from flm.main._flm_args_schema import type_to_json_schema, function_json_schema, get_args_schema_feature



class TestPrimitiveTypes(unittest.TestCase):

    def test_int(self):
        self.assertEqual(type_to_json_schema(int), {"type": "integer"})

    def test_float(self):
        self.assertEqual(type_to_json_schema(float), {"type": "number"})

    def test_str(self):
        self.assertEqual(type_to_json_schema(str), {"type": "string"})

    def test_bool(self):
        self.assertEqual(type_to_json_schema(bool), {"type": "boolean"})


class TestListAndSequence(unittest.TestCase):

    def test_bare_list(self):
        result = type_to_json_schema(list)
        self.assertEqual(result["type"], "array")

    def test_list_of_str(self):
        self.assertEqual(type_to_json_schema(list[str]), {
            "type": "array",
            "items": {"type": "string"},
        })

    def test_sequence_of_int(self):
        self.assertEqual(type_to_json_schema(Sequence[int]), {
            "type": "array",
            "items": {"type": "integer"},
        })


class TestTuple(unittest.TestCase):

    def test_bare_tuple(self):
        self.assertEqual(type_to_json_schema(tuple), {"type": "array"})

    def test_tuple_of_types(self):
        result = type_to_json_schema(tuple[str, int])
        self.assertEqual(result["type"], "array")
        self.assertEqual(result["prefixItems"], [
            {"type": "string"},
            {"type": "integer"},
        ])


class TestDictAndMapping(unittest.TestCase):

    def test_bare_dict(self):
        result = type_to_json_schema(dict)
        self.assertEqual(result["type"], "object")

    def test_dict_str_int(self):
        self.assertEqual(type_to_json_schema(dict[str, int]), {
            "type": "object",
            "additionalProperties": {"type": "integer"},
        })

    def test_mapping_str_float(self):
        self.assertEqual(type_to_json_schema(Mapping[str, float]), {
            "type": "object",
            "additionalProperties": {"type": "number"},
        })


class TestOptionalAndUnion(unittest.TestCase):

    def test_optional_str(self):
        result = type_to_json_schema(Optional[str])
        self.assertEqual(result, {
            "anyOf": [{"type": "null"}, {"type": "string"}],
        })

    def test_union(self):
        result = type_to_json_schema(typing.Union[int, str])
        self.assertEqual(result, {
            "anyOf": [{"type": "integer"}, {"type": "string"}],
        })


class TestTypedDict(unittest.TestCase):

    def test_all_required(self):
        class AllReq(TypedDict):
            name: str
            count: int

        result = type_to_json_schema(AllReq)
        self.assertEqual(result["type"], "object")
        self.assertEqual(result["properties"]["name"], {"type": "string"})
        self.assertEqual(result["properties"]["count"], {"type": "integer"})
        self.assertTrue("required" in result)
        self.assertEqual(sorted(result["required"]), ["count", "name"])
        self.assertEqual(result["additionalProperties"], False)

    def test_all_optional(self):
        class AllOpt(TypedDict, total=False):
            x: int
            y: str

        result = type_to_json_schema(AllOpt)
        self.assertEqual(result["type"], "object")
        self.assertEqual(result["properties"]["x"], {"type": "integer"})
        self.assertEqual(result["properties"]["y"], {"type": "string"})
        self.assertTrue("required" not in result)
        self.assertEqual(result["additionalProperties"], False)

    def test_all_optional_in_mapping_optional(self):
        class AllOpt(TypedDict, total=False):
            x: int
            y: str

        result = type_to_json_schema( Mapping[str,AllOpt] | None )
        print("DEBUG: result = ", repr(result))
        self.assertEqual(result["anyOf"][0]["type"], "null")
        self.assertEqual(result["anyOf"][1]["type"], "object")
        self.assertEqual(result["anyOf"][1]["additionalProperties"], {
            "type": "object",
            "additionalProperties": False,
            "properties": {
                "x": {"type": "integer"},
                "y": {"type": "string"},
            }
        })
        
    def test_nested_typeddict(self):
        class Inner(TypedDict):
            val: int

        class Outer(TypedDict):
            child: Inner

        result = type_to_json_schema(Outer)
        inner_schema = result["properties"]["child"]
        self.assertEqual(inner_schema["type"], "object")
        self.assertEqual(inner_schema["properties"]["val"], {"type": "integer"})
        self.assertEqual(inner_schema["required"], ["val"])

    def test_typeddict_with_sequence_field(self):
        class WithSeq(TypedDict):
            items: Sequence[str]

        result = type_to_json_schema(WithSeq)
        self.assertEqual(result["properties"]["items"], {
            "type": "array",
            "items": {"type": "string"},
        })

    def test_typeddict_with_optional_field(self):
        class WithOpt(TypedDict, total=False):
            tag: Optional[str]

        result = type_to_json_schema(WithOpt)
        self.assertEqual(result["properties"]["tag"], {
            "anyOf": [{"type": "null"}, {"type": "string"}],
        })


class TestFallback(unittest.TestCase):

    def test_unknown_type(self):
        self.assertEqual(type_to_json_schema(typing.Any), {})


class TestFunctionJsonSchema(unittest.TestCase):

    def test_simple_function(self):
        def fn(a: int, b: str = "hi"):
            pass

        result = function_json_schema(fn)
        self.assertEqual(result["type"], "object")
        self.assertEqual(result["properties"]["a"], {"type": "integer"})
        self.assertEqual(result["properties"]["b"], {"type": "string"})
        self.assertEqual(result["required"], ["a"])
        self.assertEqual(result["additionalProperties"], False)


# ---

class TestGetArgsSchemaFeature(unittest.TestCase):

    def test_class_init(self):
        class MyClass:
            def __init__(self, x: int, y: str = "default"):
                pass
            DocumentManager = None
            RenderManager = None

        result = get_args_schema_feature(MyClass)
        self.assertTrue("init" in result)
        self.assertEqual(set(result.keys()), set(['init', 'render_manager_initialize', 'document_manager_initialize']))
        init_schema = result["init"]
        self.assertEqual(init_schema["properties"]["x"], {"type": "integer"})
        self.assertEqual(init_schema["properties"]["y"], {"type": "string"})
        self.assertEqual(init_schema["required"], ["x"])



if __name__ == '__main__':
    unittest.main()
