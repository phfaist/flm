
import collections.abc
import inspect
import types
import typing
from typing import (
    TypedDict,
    get_type_hints, get_origin, get_args, is_typeddict,
)



def type_to_json_schema(tp):
    """
    Convert Python type hint to JSON schema fragment.
    """
    origin = get_origin(tp)
    args = get_args(tp)

    if tp is int:
        return {"type": "integer"}
    if tp is float:
        return {"type": "number"}
    if tp is str:
        return {"type": "string"}
    if tp is bool:
        return {"type": "boolean"}
    if tp is list or origin is list or origin is collections.abc.Sequence:
        item_type = args[0] if args else typing.Any
        return {
            "type": "array",
            "items": type_to_json_schema(item_type),
        }
    if tp is tuple or origin is tuple:
        if args:
            return {
                "type": "array",
                "prefixItems": [type_to_json_schema(a) for a in args
                                if a is not Ellipsis],
            }
        return {"type": "array"}
    if tp is dict or origin is dict or origin is collections.abc.Mapping:
        value_type = args[1] if len(args) == 2 else typing.Any
        return {
            "type": "object",
            "additionalProperties": type_to_json_schema(value_type),
        }
    if origin is typing.Union or isinstance(tp, types.UnionType):
        # Optional[T] = Union[T, None]
        non_none = [a for a in args if a is not type(None)]
        has_none = len(non_none) < len(args)
        schemas = [type_to_json_schema(a) for a in non_none]
        if has_none:
            schemas = [{"type": "null"}] + schemas
        if len(schemas) == 1:
            return schemas[0]
        return {"anyOf": schemas}

    if isinstance(tp, type) and is_typeddict(tp):
        hints = get_type_hints(tp)
        properties = {}
        for key, val_type in hints.items():
            properties[key] = type_to_json_schema(val_type)
        required = sorted(tp.__required_keys__)
        schema = {
            "type": "object",
            "properties": properties,
            "additionalProperties": False,
        }
        if required:
            schema["required"] = required
        return schema

    # fallback
    return {}


def function_json_schema(fn):
    sig = inspect.signature(fn)
    hints = get_type_hints(fn)

    properties = {}
    required = []
    additionalProperties = False

    for name, param in sig.parameters.items():
        if name == "self":
            continue

        if param.kind == inspect.Parameter.VAR_POSITIONAL:
            # skip "*args"
            continue
        if param.kind == inspect.Parameter.VAR_KEYWORD:
            # have **kwargs, so allow additional properties
            additionalProperties = {}
            continue

        hint = hints.get(name, typing.Any)
        properties[name] = type_to_json_schema(hint)

        if param.default is inspect.Parameter.empty:
            required.append(name)

    return {
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": additionalProperties,
    }


def get_public_attr_types(cls):
    hints = get_type_hints(cls)
    return {
        name: tp
        for name, tp in hints.items()
        if not name.startswith("_")
    }

def class_typed_attributes_json_schema(cls):
    hints = get_public_attr_types(cls)
    tp = TypedDict(f"Type{cls.__name__}Dict", hints, total=False)
    return type_to_json_schema(tp)


def get_args_schema_feature(cls):
    # inspect the class' constructor arguments.
    return {
        'init':
            function_json_schema(cls.__init__) if hasattr(cls, '__init__') else False,
        'document_manager_initialize':
            function_json_schema(cls.DocumentManager.initialize)
            if cls.DocumentManager is not None else False,
        'render_manager_initialize':
            function_json_schema(cls.RenderManager.initialize)
            if cls.RenderManager is not None else False,
    }

