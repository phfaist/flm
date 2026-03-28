

# Transcrypt-compatible typing annotations.  Make specific annotations
# for python, and provide simple working placeholders for Typescript.

# The following block will be patched & automatically replaced
# by "from ._typing_helpers_transcript import *" by our python-to-js scripts
#
### BEGINPATCH_FLM_PYTHON_TYPING
from typing import Any, Type, TypedDict as _TypedDict, Optional as _Optional, Protocol as _Protocol
from collections.abc import Sequence, Mapping, Set, Hashable

# Type that provides
class TypeCallableSpecBase(_Protocol):
    def get_node_parser(self, token) -> Any:
        ...

class TypeDictWithLatexContextDefinitions(_TypedDict, total=False):
    macros : Sequence[TypeCallableSpecBase]
    environments : Sequence[TypeCallableSpecBase]
    specials : Sequence[TypeCallableSpecBase]

### ENDPATCH_FLM_PYTHON_TYPING



#
# Common to both Python & Transcrypt/JS:
#

TypeNodeId = Hashable

