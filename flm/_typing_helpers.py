

# Transcrypt-compatible typing annotations.  Make specific annotations
# for python, and provide simple working placeholders for Typescript.

# The following block will be patched & automatically replaced
# by "from ._typing_helpers_transcript import *" by our python-to-js scripts
#
### BEGINPATCH_FLM_PYTHON_TYPING
from typing import (
    Any,
    Type,
    Literal,
    Callable,
    TypedDict as _TypedDict,
    Optional as _Optional,
    Protocol as _Protocol,
    Union as _Union
)
from collections.abc import Sequence, Mapping, Set, Hashable


# Type that provides get_node_parser() - should be sufficient for simple type checking
class TypeCallableSpecBase(_Protocol):
    def get_node_parser(self, token) -> Any:
        ...

class TypeDictWithLatexContextDefinitions(_TypedDict, total=False):
    macros : Sequence[TypeCallableSpecBase]
    environments : Sequence[TypeCallableSpecBase]
    specials : Sequence[TypeCallableSpecBase]


class TypeRenderContext(_Protocol):
    _flmtyping_is : Literal['FLMRenderContext']

class TypeFLMDocument(_Protocol):
    _flmtyping_is : Literal['FLMDocument']

class TypeFragmentRenderer(_Protocol):
    _flmtyping_is : Literal['FragmentRendererBase']

class TypeCounterFormatter(_TypedDict, total=False):
    _flmtyping_is : Literal['CounterFormatter']


# --- format_num specs ---

TypeFormatNumName = Literal[
    'alph', 'Alph', 'roman', 'Roman', 'arabic',
    'fnsymbol', 'unicodesuperscript', 'unicodesubscript',
]

class TypeFormatNumTemplate(_TypedDict):
    template: str

TypeFormatNumFn = Callable[[int], str]

TypeFormatNumSpec = _Union[TypeFormatNumName, TypeFormatNumTemplate, TypeFormatNumFn]


# --- subnum specs ---

class TypeSubnumResult(_TypedDict):
    formatted: str
    prefix: str

TypeSubnumFormatFn = Callable[[int], TypeSubnumResult]

class TypeSubnumFormatDict(_TypedDict):
    format_num: TypeFormatNumSpec
    prefix: str

TypeFormatSubnumSpec = _Union[TypeSubnumFormatFn, TypeSubnumFormatDict]


# --- prefix_display ---

class TypePrefixDisplayVariant(_TypedDict, total=False):
    singular: str
    plural: str

class TypePrefixDisplaySpec(_TypedDict, total=False):
    singular: str
    plural: str
    capital: TypePrefixDisplayVariant

TypePrefixDisplayInput = _Union[None, str, TypePrefixDisplaySpec]


TypeStandardCounterFormattersDict = dict[TypeFormatNumName, TypeFormatNumFn]


# --- join_spec ---

TypeJoinSpecName = Literal['default', 'compact']

TypeJoinSpecDict = _TypedDict(
    # need this syntax because of the "and" keyword
    "TypeJoinSpecDict",
    {
        "one_pre": str,
        "one_post": str,
        "pair_pre": str,
        "pair_mid": str,
        "pair_post": str,
        "range_pre": str,
        "range_mid": str,
        "range_pairmid": str,
        "range_post": str,
        "list_pre": str,
        "list_mid": str,
        "list_midlast": str,
        "list_post": str,
        "and": str,
        "sep": str,
        "endash": str,
        "empty": str,
    },
    total=False
)

TypeJoinSpecInput = _Union[None, TypeJoinSpecName, TypeJoinSpecDict]


# --- CounterFormatterSpecDict ---

class TypeCounterFormatterSpecDict(_TypedDict, total=False):
    format_num: TypeFormatNumSpec
    prefix_display: TypePrefixDisplayInput
    delimiters: tuple[str, str]
    join_spec: TypeJoinSpecInput
    name_in_link: bool
    repeat_numprefix_in_range: bool
    counter_formatter_id: str
    subnums_format_nums: Sequence[TypeFormatSubnumSpec]


# --- Top-level input to build_counter_formatter() ---

TypeCounterFormatterInput = _Union[
    None,
    TypeCounterFormatter,
    TypeFormatNumName,
    TypeFormatNumTemplate,
    TypeCounterFormatterSpecDict,
]


# --- Enumeration-specific ---

TypeEnumerationCounterFormatterSingleSpec = _Union[
    TypeFormatNumSpec,
    str,  # tag template, e.g. "(a)", "1.", bullet chars
]

TypeEnumerationCounterFormatterInput = Sequence[TypeEnumerationCounterFormatterSingleSpec]

### ENDPATCH_FLM_PYTHON_TYPING



#
# Common to both Python & Transcrypt/JS:
#

TypeNodeId = Hashable

