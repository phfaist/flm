
from pylatexenc.latexnodes import (
    LatexArgumentSpec,
    ParsedArguments,
    ParsingStateDelta,
)
from pylatexenc.latexnodes.parsers import (
    LatexParserBase,
)
from pylatexenc.latexnodes.nodes import (
    latex_node_types,
    LatexNodeList
)
from pylatexenc.macrospec import (
    CallableSpec,
    LatexContextDb,
    MacroSpec, EnvironmentSpec, SpecialsSpec,
)

from .flmenvironment import (
    FLMLatexWalker,
    FLMParsingState,
    FLMParsingStateDeltaSetBlockLevel,
)

from .flmspecinfo import (
    FLMSpecInfo,
)

from .flmfragment import (
    FLMFragment,
)



### BEGINPATCH_UNIQUE_OBJECT_ID
fn_unique_object_id = id
### ENDPATCH_UNIQUE_OBJECT_ID




### BEGINPATCH_IMPORT_FLMSPECINFO_CLASS
import importlib
def _import_class(fullclsname, restype):
    modname, clsname = fullclsname.rsplit(':', 1)
    mod = importlib.import_module(modname)
    return getattr(mod, clsname)
### ENDPATCH_IMPORT_FLMSPECINFO_CLASS

def _fullclassname(clsobj):
    # It would have been better to use __qualname__, but the latter is not
    # supported by Transcrypt.  So we fix here that serializable classes must be
    # top-level class definitions in their module.  (It can be any internal
    # module, if necessary.)
    return clsobj.__module__ + ':' + clsobj.__name__


# ------------------------------------------------------------------------------


class _FakeDataLoadedFLMLatexWalker:
    def __init__(self, s):
        self.s = s
        self._fields = ('s',)

    def __repr__(self):
        return f"{self.__class__.__name__}(s={repr(self.s)})"





latex_node_types_dict = {
    c.__name__: c
    for c in latex_node_types
}

_objtypes = {
    c.__name__: c
    for c in [
            FLMParsingState,
            ParsedArguments,
            LatexArgumentSpec,
            ParsingStateDelta,
            FLMParsingStateDeltaSetBlockLevel,
    ]
}




# ------------------------------------------------------------------------------


# class _FakeDataFLMSpecInfoDumper:
#     def __init__(self, x):
#         self._proxy_object = x
#         self.flmspecinfo_type = x.__class__.__name__
#         self._fields = tuple(list(x._fields) + ['flmspecinfo_type'])


# ------------------------------------------------------------------------------

_dump_version = 2



class _Skip:
    pass


_skip_types = (
    #LatexArgumentSpec, CallableSpec,
    LatexContextDb,
    LatexParserBase,

    # non-FLMSpecInfo spec classes, normally only used for marking nodes to be
    # post-processed and not rendered (e.g., \item, \label, etc.)
    MacroSpec, EnvironmentSpec, SpecialsSpec,
)


class FLMDataDumper:
    r"""
    Create JSON data dumps of FLM compiled objects (fragments).
    """

    def __init__(self, *, environment):
        self.environment = environment
        self.clear()

    def clear(self):
        self.data = {
            'dumps': {},
            'resources': {},
            '_dump': {
                'version': _dump_version,
            }
        }
        
    def get_data(self):
        return self.data

    def add_object_dump(self, key, obj):
        dump = self._make_object_dump(obj, dumping_state={'object': obj})
        self.data['dumps'][key] = dump

    # ---

    def _make_object_dump(self, obj, *, dumping_state, type_name=None):

        fieldnames = set(obj._fields)
        for fieldname in dir(obj): # dir() is supported by Transcrypt
            if fieldname.startswith('flm'):
                fieldnames.add(fieldname)

        objdata = {
            '$type': type_name if type_name is not None else obj.__class__.__name__,
        }
        
        def get_obj_attr(fieldname):
            if hasattr(obj, fieldname):
                return getattr(obj, fieldname)
            if hasattr(obj, '_proxy_object') and hasattr(obj._proxy_object, fieldname):
                return getattr(obj._proxy_object, fieldname)
            raise ValueError("Invalid object field: " + repr(fieldname) + " in " + repr(obj))
        for field in sorted(fieldnames):
            val = self._make_dump(get_obj_attr(field), dumping_state=dumping_state)
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

        if isinstance(x, dict):
            return { k: self._make_dump(v, dumping_state=dumping_state)
                     for k, v in x.items() }

        if x is self.environment:
            return { "$flmenv": "environment" }

        if x is self.environment.parsing_state:
            return { "$flmenv": "parsing_state" }

        if isinstance(x, FLMLatexWalker):
            return self._make_resource(
                'FLMLatexWalker',
                x,
                _FakeDataLoadedFLMLatexWalker(x.s),
                dumping_state=dumping_state,
            )
        
        if isinstance(x, FLMSpecInfo):
            return self._make_resource(
                'FLMSpecInfo',
                x,
                x, #_FakeDataFLMSpecInfoDumper(x), ## -- not necessary
                dumping_state=dumping_state,
                restype_dumptype=_fullclassname(x.__class__)
            )

        if isinstance(x, FLMParsingState):
            return self._make_resource(
                'FLMParsingState',
                x, # actual object (used for id/references identification)
                x, # object to dump (will pass through _make_object_dump())
                dumping_state=dumping_state,
            )

        if isinstance(x, latex_node_types):
            return self._make_resource(
                'LatexNode',
                x, # actual object (used for id/references identification)
                x, # object to dump (will pass through _make_object_dump())
                dumping_state=dumping_state,
                restype_dumptype=x.__class__.__name__,
            )

        if isinstance(x, _skip_types):
            return _Skip

        if hasattr(x, '_fields'):
            return self._make_object_dump(x, dumping_state=dumping_state)

        if x is None:
            return None

        if isinstance(x, (str, bool, int, float)):
            return x

        raise ValueError(f"Cannot dump value {repr(x)} of unsupported type")

    def _make_resource(self, restype, y, ydata, *, restype_dumptype=None, dumping_state):
        if restype not in self.data['resources']:
            self.data['resources'][restype] = {}

        reskey = str(fn_unique_object_id(y))
        if reskey not in self.data['resources'][restype]:
            # already mark this object as being dumped, in case we recursively
            # encounter a reference to this object while dumping this object
            # itself.
            self.data['resources'][restype][ reskey ] = '$currently-dumping'

            ydata_dump = self._make_object_dump(
                ydata, dumping_state=dumping_state,
                type_name=(restype_dumptype if restype_dumptype is not None else restype)
            )

            self.data['resources'][restype][ reskey ] = ydata_dump

        return { '$restype': restype, '$reskey': reskey }
    


class FLMDataLoadNotSupported:
    r"""
    This object is stored in non-serializable properties (say
    latex_context=) rather than `None`, in order to avoid having the user wonder
    why latex_context is `None`.
    """
    pass



class FLMDataLoader:
    r"""
    Read JSON data dumps of FLM compiled objects (fragments).
    """

    def __init__(self, data, *, environment):
        self.data = data
        self.environment = environment

        if self.data['_dump']['version'] != _dump_version:
            raise RuntimeError(
                f"Dump version mismatch: {self.data['_dump']['version']}, "
                f"expected {_dump_version}"
            )

        self._loaded_resources = {}
        if 'resources' in self.data and self.data['resources']:
            for restype in self.data['resources']:
                self._loaded_resources[restype] = {}


    def get_keys(self):
        return list(self.data['dumps'].keys())

    def get_object_dump(self, key):
        data = self.data['dumps'][key]
        return self._load_from_data(data)

    # ---

    def _load_from_data(self, data):

        if data is None:
            return None

        if isinstance(data, list):
            return [ self._load_from_data(item) for item in data ]

        # avoid " isinstance(data, dict) " because the data might be a raw JS
        # object when using Transcrypt.

        special = None
        try:
            special = data['$flmenv']
        except Exception: pass
        if special:
            return self._flmenv_object(data['$flmenv'], data)

        special = None
        try:
            special = data['$skip']
        except Exception: pass
        if special:
             return FLMDataLoadNotSupported
            
        special = None
        try:
            special = data['$restype']
        except Exception: pass
        if special:
            return self._load_resource(special, data['$reskey'])
            
        special = None
        try:
            special = data['$type']
        except Exception: pass
        if special:
            datad = dict(data)
            thetype = datad.pop('$type')
            return self._load_object( thetype, datad )

        return data # should be a simple scalar -- string, int, bool, etc.


    def _flmenv_object(self, flmenv_what, data):
        if flmenv_what == '':
            return self.environment
        if flmenv_what == 'parsing_state':
            return self.environment.parsing_state
        raise ValueError("Unknown/invalid flmenv: " + repr(data))


    def _load_resource(self, restype, reskey):

        if restype not in self._loaded_resources:
            self._loaded_resources[restype] = {}

        if reskey not in self._loaded_resources[restype]:

            self._loaded_resources[restype][reskey] = \
                self._load_resource_from_data(restype, reskey)

        # resource object is loaded, return it
        return self._loaded_resources[restype][reskey]

    def _load_resource_from_data(self, restype, reskey):

        if restype not in self.data['resources']:
            raise ValueError(f"Invalid internal resource reference type {restype}")
        if reskey not in self.data['resources'][restype]:
            raise ValueError(f"Invalid internal resource reference key {restype}/{reskey}")

        resdata = self.data['resources'][restype][reskey]

        if restype == 'FLMLatexWalker':
            return _FakeDataLoadedFLMLatexWalker(resdata['s'])

        if restype == 'FLMParsingState':
            return self._load_object(
                restype,
                dict(resdata, latex_context=FLMDataLoadNotSupported)
            )

        if restype == 'FLMSpecInfo':
            resdata2 = dict(resdata)
            the_type = resdata2.pop('$type')
            # import the correct class
            the_class = _import_class(the_type, restype=restype)
            # fields
            return the_class(**resdata2)

        if restype == 'LatexNode':
            resdata2 = dict(resdata)
            the_type = resdata2.pop('$type')
            return self._load_object(
                the_type,
                resdata2,
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

        data = dict(data) # for Transcrypt, in case this is a raw JS object

        args = {
            k: self._load_from_data(v)
            for (k, v) in data.items()
            if not k.startswith('$')
        }

        # print(f"DEBUG: flmdump: Loading ‘{objtype}’ object with args = {repr(args)}")
        obj = ObjTypeFn(**args)
        return obj

    def _make_node_instance(self, nodetype, kwargs):

        # note, we don't need to to use latexwalker.make_node() and
        # latexwalker.make_nodelist() because any postprocessing has already
        # been saved as node.flm_* attributes on the node object.

        base_kwargs = {}
        attrib_kwargs = {}
        for k, v in kwargs.items():
            if k.startswith('flm'):
                attrib_kwargs[k] = v
            else:
                base_kwargs[k] = v

        if nodetype == 'LatexNodeList':
            ObjTypeFn = LatexNodeList
            finalize_fn = self.environment.finalize_nodelist
        else:
            ObjTypeFn = latex_node_types_dict[nodetype]
            finalize_fn = self.environment.finalize_node

        node = ObjTypeFn(**base_kwargs)
        for k, v in attrib_kwargs.items():
            setattr(node, k, v)

        node = finalize_fn(node)

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
