
from pylatexenc.latexnodes import (
    LatexArgumentSpec,
    ParsedArguments
)
from pylatexenc.latexnodes.nodes import (
    latex_node_types,
    LatexNodeList
)
from pylatexenc.macrospec import (
    MacroSpec, EnvironmentSpec, SpecialsSpec, LatexContextDb
)

from .flmenvironment import (
    FLMLatexWalker,
    FLMParsingState,
)

from .flmfragment import (
    FLMFragment
)



### BEGINPATCH_UNIQUE_OBJECT_ID
fn_unique_object_id = id
### ENDPATCH_UNIQUE_OBJECT_ID


# ---------------------------------------------------------------------------------------


class _FakeDataLoadedFLMLatexWalker:
    def __init__(self, s):
        self.s = s
        self._fields = ('s',)

    def __repr__(self):
        return f"{self.__class__.__name__}(s={self.s!r})"




# ---------------------------------------------------------------------------------------

_dump_version = 1


class _Skip:
    pass


_skip_types = (
    LatexArgumentSpec, MacroSpec, EnvironmentSpec, SpecialsSpec, LatexContextDb,
)


class FLMDataDumper:
    def __init__(self, *, environment):
        self.environment = environment
        self.clear()

    def clear(self):
        self.data = {
            'objects': {},
            'resources': {},
            '_dump': {
                'version': _dump_version,
            }
        }
        
    def get_data(self):
        return self.data

    def add_dump(self, key, obj):
        dump = self._make_object_dump(obj, dumping_state={'object': obj})
        self.data['objects'][key] = dump

    # ---

    def _make_object_dump(self, obj, *, dumping_state, type_name=None):

        fieldnames = set(obj._fields)
        for fieldname in dir(obj): # dir() is supported by Transcrypt
            if fieldname.startswith('flm'):
                fieldnames.add(fieldname)

        objdata = {
            '$type': type_name if type_name is not None else obj.__class__.__name__,
        }
        for field in sorted(fieldnames):
            val = self._make_dump(getattr(obj, field), dumping_state=dumping_state)
            if val is _Skip:
                val = { '$skip': True }
            objdata[field] = val

        return objdata

    def _make_dump(self, x, *, dumping_state):

        if isinstance(x, (tuple,list)):
            result = []
            for item in x:
                value = self._make_dump(item, dumping_state=dumping_state)
                if value is _Skip:
                    value = None
                result.append(value)
            return result

        if isinstance(x, FLMLatexWalker):
            return self._make_resource(
                'FLMLatexWalker',
                x,
                _FakeDataLoadedFLMLatexWalker(x.s),
                dumping_state=dumping_state,
            )
        
        if isinstance(x, FLMParsingState):
            return self._make_resource(
                'FLMParsingState',
                x, # actual object (used for id/references identification)
                x, # object to dump (will pass through _make_object_dump())
                dumping_state=dumping_state
            )

        if isinstance(x, _skip_types):
            return _Skip

        if hasattr(x, '_fields'):
            return self._make_object_dump(x, dumping_state=dumping_state)

        if x is None:
            return None

        if isinstance(x, (str, bool, int, float)):
            return x

        raise ValueError(f"Cannot dump value {x!r} of unsupported type")

    def _make_resource(self, restype, y, ydata, *, dumping_state):
        if restype not in self.data['resources']:
            self.data['resources'][restype] = {}

        reskey = str(fn_unique_object_id(y))
        if reskey not in self.data['resources'][restype]:
            ydata_dump = self._make_object_dump(
                ydata, dumping_state=dumping_state, type_name=restype
            )
            self.data['resources'][restype][ reskey ] = ydata_dump

        return { '$restype': restype, '$reskey': reskey }
    


# store this object in non-serializable properties (say latex_context=) rather
# than `None` and having the user wondering why latex_context is `None`
class FLMDataLoadNotSupported:
    pass


latex_node_types_dict = {
    c.__name__: c
    for c in latex_node_types
}

_objtypes = {
    c.__name__: c
    for c in [
            FLMParsingState,
            ParsedArguments,
    ]
}

class FLMDataLoader:
    def __init__(self, data, *, environment):
        self.data = data
        self.environment = environment

        if self.data['_dump']['version'] != _dump_version:
            raise RuntimeError(
                f"Dump version mismatch: {self.data['_dump']['version']}, "
                f"expected {_dump_version}"
            )

    def get_keys(self):
        return list(self.data['objects'].keys())

    def get_object(self, key):
        data = self.data['objects'][key]
        return self._load_from_data(data)

    # ---

    def _load_from_data(self, data):
        if isinstance(data, list):
            return [ self._load_from_data(item) for item in data ]

        if isinstance(data, dict):
            if '$skip' in data and data['$skip'] is True:
                return FLMDataLoadNotSupported
            if '$restype' in data:
                return self._load_resource(data['$restype'], data['$reskey'])
            if '$type' in data:
                return self._load_object( data.pop('$type'), data)

        return data # should be a simple scalar -- string, int, bool, etc.

    def _load_resource(self, restype, reskey):
        resdata = self.data['resources'][restype][reskey]
        if restype == 'FLMLatexWalker':
            return _FakeDataLoadedFLMLatexWalker(resdata['s'])
        if restype == 'FLMParsingState':
            return self._load_object(
                restype,
                dict(resdata, latex_context=FLMDataLoadNotSupported)
            )
        raise ValueError(f"Unknown data resource type to load: {restype}")

    def _load_object(self, objtype, data):

        if objtype == 'FLMFragment':
            ObjTypeFn = self._make_fragment
        elif objtype in latex_node_types_dict or objtype == 'LatexNodeList':
            ObjTypeFn = lambda **kwargs: self._make_node_instance(objtype, kwargs)
        elif objtype in _objtypes:
            ObjTypeFn = _objtypes[objtype]
        else:
            raise ValueError(f"Unknown object type ‘{objtype}’ for data loading")

        args = {
            k: self._load_from_data(v)
            for (k, v) in data.items()
            if not k.startswith('$')
        }

        # print(f"DEBUG: flmdump: Loading ‘{objtype}’ object with args = {repr(args)}")
        obj = ObjTypeFn(**args)
        return obj

    def _make_node_instance(self, nodetype, kwargs):
        base_kwargs = {}
        attrib_kwargs = {}
        for k, v in kwargs.items():
            if k.startswith('flm'):
                attrib_kwargs[k] = v
            else:
                base_kwargs[k] = v

        if nodetype == 'LatexNodeList':
            ObjTypeFn = LatexNodeList
        else:
            ObjTypeFn = latex_node_types_dict[nodetype]

        node = ObjTypeFn(**base_kwargs)
        for k, v in attrib_kwargs.items():
            setattr(node, k, v)

        return node
        
    def _make_fragment(self, **kwargs):
        nodelist = kwargs.pop('nodes')
        flm_text = kwargs.pop('flm_text')
        return FLMFragment(
            flm_text=nodelist,
            environment=self.environment,
            _flm_text_if_loading_nodes=flm_text,
            **kwargs,
        )
