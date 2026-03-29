import sys
import os # os.pathsep
import os.path
import re
import copy
import logging
logger = logging.getLogger(__name__)

from tempfile import TemporaryDirectory

from typing import Any, Optional
from collections.abc import Mapping
from dataclasses import dataclass

import frontmatter
import yaml
import jsonschema

from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from ._util import abbrev_value_str

from flm import flmenvironment

from ._flm_args_schema import (
    function_json_schema, get_args_schema_feature, type_to_json_schema,
    class_typed_attributes_json_schema
)


# ---

class FLMMainRunError(Exception):
    r"""
    Exception raised by the FLM main run pipeline when a user-facing error
    occurs (e.g. missing template, unresolvable feature class).

    Carries a short *message* suitable for display and optional *details*
    with diagnostic context (e.g. Python search path).

    :param message: Human-readable error summary.
    :param details: Optional extended diagnostic information.
    """
    def __init__(self, message, details=None):
        super().__init__(message)
        self._message = message.strip()
        self._details = details.strip()

    def message(self):
        return self._message

    def details(self):
        return self._details

class ResourceAccessorBase:
    r"""
    Abstract base interface for accessing templates, importing feature and
    workflow class instances, and reading filesystem resources.

    Subclasses must implement :meth:`import_class`, :meth:`read_file`,
    :meth:`open_file_object_context`, :meth:`file_exists`, and
    :meth:`dir_exists`.  See the concrete subclass
    :py:class:`flm.main.ResourceAccessor`.
    """


    template_exts = ['', '.yaml', '.yml', '.json']

    template_path = [
        None
    ]

    def get_template_info_file_name(self, template_prefix, template_name, flm_run_info):

        cwd = flm_run_info.get('cwd', None)

        for tpath in self.template_path:

            if tpath is None:
                tpath = cwd # might still be `None`

            for t_ext in self.template_exts:
                tfullname = os.path.join(template_prefix, f"{template_name}{t_ext}")
                if self.file_exists(tpath, tfullname, 'template_info', flm_run_info):
                    return tpath, tfullname

        raise FLMMainRunError(
            f"Template not found: ‘{template_name}’.  "
            f"Template path is = {repr(self.template_path)}"
        )

    @classmethod
    def get_cwd_for_resource_info(cls, resource_info, flm_run_info):
        cwd = flm_run_info.get('cwd', '.')
        #print(f"DOCUMENT cwd = ", cwd)
        if resource_info is not None:
            relative_to_source_path = resource_info.source_path
            if relative_to_source_path is not None and relative_to_source_path:
                r_cwd = os.path.dirname(os.path.join(cwd, relative_to_source_path))
                if r_cwd:
                    return r_cwd
        return cwd

    def find_in_search_paths(self, search_paths, fname, ftype, flm_run_info, resource_info) -> tuple[str, str]:
        r"""
        Locate a file by searching through a list of directory paths.

        Each search path is resolved relative to the working directory
        derived from *resource_info* and *flm_run_info*.

        :param search_paths: Ordered list of directory paths to search.
        :param fname: Filename to look for.
        :param ftype: A descriptive file-type string (e.g. ``'template_info'``).
        :param flm_run_info: The run-info dictionary.
        :param resource_info: A :class:`ResourceInfo` instance (or ``None``).
        :returns: A ``(search_path, fname)`` tuple for the first match.
        :raises FLMMainRunError: If the file is not found in any search path.
        """

        cwd = self.get_cwd_for_resource_info(resource_info, flm_run_info)

        #print(f'cwd = {cwd}, search_paths = {search_paths}')

        for search_path in search_paths:

            if cwd is not None:
                search_path = os.path.join(cwd, search_path)

            if self.file_exists(search_path, fname, ftype, flm_run_info):
                return search_path, fname

        raise FLMMainRunError(
            f"File not found: ‘{fname}’. Search path was = {repr(search_paths)}, "
            f"relative to ‘{cwd}’"
        )

    def import_class(self, fullname, *, default_classnames=None, default_prefix=None,
                     flm_run_info=None) -> tuple[Any,Any]: # (ModuleObject, ClassObject)
        r"""
        Import and return a ``(module, class)`` pair for the given fully or
        partially qualified class name.

        :param fullname: Dotted module path, optionally including the class
            name.
        :param default_classnames: Candidate class attribute names to try
            when *fullname* does not include a class name.
        :param default_prefix: Module prefix to prepend when *fullname* is a
            short name.
        :param flm_run_info: The run-info dictionary.
        :returns: A ``(module_object, class_object)`` tuple.
        """
        raise RuntimeError("Must be reimplemented by subclasses!")

    def read_file(self, fpath, fname, ftype, flm_run_info, binary=False) -> str|bytes:
        r"""
        Read and return the contents of a file.

        :param fpath: Directory path containing the file.
        :param fname: Filename (joined with *fpath*).
        :param ftype: Descriptive file-type string.
        :param flm_run_info: The run-info dictionary.
        :param binary: If ``True``, return raw bytes; otherwise return a
            decoded string.
        :returns: File contents as :class:`str` or :class:`bytes`.
        """
        raise RuntimeError("Must be reimplemented by subclasses!")

    def open_file_object_context(self, fpath, fname, ftype, flm_run_info, binary=False) -> Any:
        raise RuntimeError("Must be reimplemented by subclasses!")

    def file_exists(self, fpath, fname, ftype, flm_run_info) -> bool:
        r"""
        Check whether a file exists.

        :param fpath: Directory path.
        :param fname: Filename (joined with *fpath*).
        :param ftype: Descriptive file-type string.
        :param flm_run_info: The run-info dictionary.
        :returns: ``True`` if the file exists, ``False`` otherwise.
        """
        raise RuntimeError("Must be reimplemented by subclasses!")

    def dir_exists(self, fpath, fname, ftype, flm_run_info) -> bool:
        r"""
        Check whether a directory exists.

        :param fpath: Parent directory path.
        :param fname: Subdirectory name (joined with *fpath*).
        :param ftype: Descriptive file-type string.
        :param flm_run_info: The run-info dictionary.
        :returns: ``True`` if the directory exists, ``False`` otherwise.
        """
        raise RuntimeError("Must be reimplemented by subclasses!")
        


# ------------------


_rx_frontmatter = re.compile(r"^-{3,}\s*$\s*", re.MULTILINE) # \s also matches newline

def parse_frontmatter_content_linenumberoffset(input_content):

    frontmatter_metadata, content = frontmatter.parse(input_content)

    # compute line number offset (it doesn't look like I can grab this from the
    # `frontmatter` module's result :/
    m = _rx_frontmatter.search(input_content) # top separator
    if m is not None:
        m = _rx_frontmatter.search(input_content, m.end()) # below the front matter
    line_number_offset = 0
    if m is not None:
        line_number_offset = input_content[:m.end()].count('\n') + 1

    return frontmatter_metadata, content, line_number_offset



# ------------------



class ResourceInfo:
    r"""
    Describes the origin of a document fragment's source content.

    By convention, :attr:`source_path` is relative to the document's root
    folder.

    :param source_path: Relative file path of the source, or ``None`` if
        the fragment does not originate from a file.
    """
    def __init__(self, source_path):
        super().__init__()
        self.source_path = source_path

        if source_path is not None:
            self._source_dirname = os.path.dirname(self.source_path)
        else:
            self._source_dirname = None

    def get_source_directory(self):
        r"""
        Return the directory portion of :attr:`source_path`, or ``None`` if
        no source path was provided.
        """
        return self._source_dirname




# ------------------


# ..........
# flm_run_info = {
#     'resource_accessor': .... # instance of ResourceAccessorBase subclass

#     'outputformat': .....
#     'workflow': ......
#     'force_block_level': None|true|false
#     'template': ....
#     'add_template_path': .....
#     'main_config': ... # fully merged config (?)
#
#     'cwd': ..... # input CWD
#     'output_cwd': ...... # reference output CWD for all FLM-processing stuff (might be temporary directory for some workflows)
#     'output_filepath': {
#         'dirname': output_dirname,
#         'basename': output_basename,
#         'jobname': output_jobname,
#         'jobnameext': output_jobnameext,
#     }
#     'input_lineno_colno_offsets': ..... # passed on to flmfragment, adjust line/col numbers
#     'metadata': ..... # to be merged into the document's metadata. Can
#                       # include information about the FLM source, etc.
# }



# ---



def import_component_class(
    resource_accessor : ResourceAccessorBase,
    componentname : str,
    default_prefix : str,
    default_classnames : list[str],
    flm_run_info : Any,
    what: str,
) -> tuple[Any,Any]:

    try:
        mod, cls = resource_accessor.import_class(
            componentname,
            default_prefix='flm.feature',
            default_classnames=['FeatureClass'],
            flm_run_info=flm_run_info
        )
        return mod, cls
    except ValueError as e:
        msg = (
f"""
Failed to locate {what} ‘{componentname}’.
"""
)
        details = (
f"""
Current python search path is {repr(sys.path)}
"""
)
        raise FLMMainRunError(msg, details)




def load_features(features_merge_configs, flm_run_info):

    main_config = flm_run_info['main_config']

    features_onoff = main_config['flm']['features']

    resource_accessor = flm_run_info['resource_accessor']

    features = []

    feature_configs = {}

    for featurename, featureconfig in features_onoff.items():

        if featureconfig is None or featureconfig is False:
            continue
        if featureconfig is True:
            featureconfig = {}

        _, FeatureClass = import_component_class(
            resource_accessor,
            featurename,
            default_prefix='flm.feature',
            default_classnames=['FeatureClass'],
            flm_run_info=flm_run_info,
            what='feature',
        )

        # re-merge the config fully from the initial merge configs, so that we
        # make sure we honor $defaults etc. properly
        
        feature_merge_configs = [
            _ensurefeatureconfig(c.get(featurename, True))
            for c in features_merge_configs
        ]

        if hasattr(FeatureClass, 'feature_default_config'):
            feature_merge_configs.append( FeatureClass.feature_default_config or {} )

        logger.debug("Feature config chain for ‘%s’ is = %r",
                     featurename, feature_merge_configs)

        featureconfig = configmerger.recursive_assign_defaults(feature_merge_configs)

        logger.debug("Instantiating feature ‘%s’ with config = %s", featurename,
                     abbrev_value_str(featureconfig, maxstrlen=512) )
        
        validate_config_for_fn_kwargs(
            _join_config_path(['flm', 'features', featurename]),
            FeatureClass.__init__,
            featureconfig,
        )

        features.append( FeatureClass(**featureconfig) )

        feature_configs[featurename] = feature_configs

    return features, feature_configs


def _ensurefeatureconfig(x):
    if x is True:
        return {}
    if x is False:
        raise ValueError("Got value ‘False’ in feature config that is being instantiated!")
    return x

#
# config
# ======
#
# flm:
#   parsing:
#     enable_dollar_math: .... etc.
#
#   features:
#     feature_name:
#       configvar1: ...
#       configvar2: ...
#     feature_name2: false | null # disable this feature
#     feature_name3: true # enable this feature with no options, same as {}
#
#   renderer:
#     html:
#       ....
#     text:
#       ....
#
#   template_path:
#     - path/here/
#     - pkg:use_import_package
#


_global_config_schema = yaml.safe_load(
r"""
type: object
properties:
    flm:
        type: object
        additionalProperties: false
        properties:
            parsing: {}
            features: {}
            renderer: {}
            workflow_config: {}
                

            default_workflow:
                type: ['string', 'null']
            default_format:
                type: ['string', 'null']

            template:
                # Either a simple string (template name) or by format
                anyOf:
                  - type: string
                  - type: object
                    additionalProperties:
                        type: string
            template_config: # keys = output formats
                type: object
                additionalProperties:
                    type: object
            template_path:
                type: array
                items:
                    type: string
""")




_dirname_here = os.path.dirname(__file__)
_builtin_default_config_yaml = os.path.join(_dirname_here, 'default_config.yaml')
with open(_builtin_default_config_yaml, encoding='utf-8') as f:
    _builtin_default_config = yaml.safe_load(f)
    _builtin_default_config['$_cwd'] = _dirname_here




@dataclass
class WorkflowEnvironmentInformation:

    environment : Optional[Any] = None

    config : Optional[Mapping] = None

    feature_configs : Optional[Mapping] = None

    flm_run_info: Optional[Mapping] = None

    workflow: Optional[Any] = None

    fragment_renderer_name: Optional[str] = None

    use_temporary_directory_output: Optional[TemporaryDirectory] = None


    def cleanup(self):
        if self.use_temporary_directory_output is not None:
            self.use_temporary_directory_output.cleanup()




def _join_config_path(parts):
    def _fmtpart(p):
        if re.match(r'^[a-zA-Z0-9_]+$', p) is not None:
            return p
        if '"' not in p:
            return f'"{p}"'
        return repr(p)
    return ".".join([
        _fmtpart(part)
        for part in parts
    ])


def _collect_leaf_errors(error):
    """Collect user-friendly leaf errors from a jsonschema ValidationError tree.

    For anyOf/oneOf nodes, discard shallow type-mismatch branches (e.g.
    'not of type null') and recurse into all remaining deeper branches.
    """
    if error.validator in ('anyOf', 'oneOf') and error.context:
        # Separate shallow type-only errors from deeper/structural errors
        deep_subs = [e for e in error.context
                     if len(list(e.path)) > 0 or e.context]
        if not deep_subs:
            # All sub-errors are at the same level with no nesting; treat
            # this anyOf/oneOf as a leaf (will be summarized by _format_leaf_error)
            return [error]
        leaves = []
        for sub in deep_subs:
            leaves.extend(_collect_leaf_errors(sub))
        return leaves if leaves else [error]

    # If this error has sub-errors from other combinators, recurse
    if error.context:
        leaves = []
        for sub in error.context:
            leaves.extend(_collect_leaf_errors(sub))
        return leaves if leaves else [error]

    return [error]


def _format_leaf_error(error):
    """Format a single leaf ValidationError into a one-line string."""
    path = list(error.absolute_path)
    if path:
        path_str = "$." + _join_config_path([str(p) for p in path])
    else:
        path_str = "$"

    msg = error.message

    # For anyOf/oneOf at the very leaf (all sub-errors are simple type checks),
    # summarize expected types concisely
    if error.validator in ('anyOf', 'oneOf') and error.context:
        types = []
        for sub in error.context:
            if sub.validator == 'type':
                types.append(sub.validator_value)
        if types:
            instance_repr = abbrev_value_str(error.instance)
            msg = f"{instance_repr}: expected {' or '.join(types)}"

    return f"  at {path_str}: {msg}"


def validate_config_for_schema(name, schema, config):

    validator = jsonschema.Draft202012Validator(schema)
    iter_errors = validator.iter_errors(instance=config)
    errors = sorted(iter_errors, key=lambda e: list(e.path))

    if not errors:
        return

    # there are errors - dump instance & schema to facilitate debugging
    logger.debug(
        "Schema validation error in ‘%s’:\nconfig=%r\nschema=%r",
        name, config, schema
    )

    lines = [f"FLM config validation error(s) in \u2018{name}\u2019:"]
    for error in errors:
        leaves = _collect_leaf_errors(error)
        for leaf in leaves:
            lines.append(_format_leaf_error(leaf))

    logger.warning("\n".join(lines) + "\n")

def validate_config_for_fn_kwargs(name, fn, config):
    schema = function_json_schema(fn)
    validate_config_for_schema(name, schema, config)

def validate_config_for_tp(name, tp, config):
    schema = type_to_json_schema(tp)
    validate_config_for_schema(name, schema, config)

def validate_config_for_class_typed_attributes(name, cls, config):
    schema = class_typed_attributes_json_schema(cls)
    validate_config_for_schema(name, schema, config)



def get_config_json_schema(
        feature_classes,
        renderer_classes,
        workflow_classes
    ):

    configschema = copy.deepcopy(_global_config_schema)

    #
    # parsing
    #

    configschema['properties']['flm']['properties']['parsing'] = function_json_schema(
        flmenvironment.standard_parsing_state
    )
    
    #
    # features
    #
    
    features_schema = {}
    for feature_name, feature_class in feature_classes.items():
        features_schema[feature_name] = {
            'anyOf': [
                { 'type': 'boolean' },
                function_json_schema( feature_class.__init__ ),
            ]
        }
    
    configschema['properties']['flm']['properties']['features'] = {
        'type': 'object',
        'additionalProperties': {},
        'properties': features_schema,
    }

    #
    # renderers
    #
    
    renderers_schema = {}
    for renderer_name, renderer_class in renderer_classes.items():
        renderers_schema[renderer_name] = class_typed_attributes_json_schema(renderer_class)

    configschema['properties']['flm']['properties']['renderer'] = {
        'type': 'object',
        'additionalProperties': {},
        'properties': renderers_schema,
    }

    #
    # workflows
    #
    
    workflows_schema = {}
    for workflow_name, workflow_class in workflow_classes.items():
        workflows_schema[workflow_name] = \
            type_to_json_schema(workflow_class.TypeWorkflowConfigDict)

    configschema['properties']['flm']['properties']['workflow_config'] = workflows_schema,

    return configschema








def load_workflow_environment(*,
                              flm_run_info,
                              run_config,
                              default_configs=None,
                              add_builtin_default_configs=True):

    logger.debug(f"load_workflow_environment: {run_config=}, {flm_run_info=}, {default_configs=} {add_builtin_default_configs=}")

    resource_accessor = flm_run_info['resource_accessor']

    # Set up the workflow to get the output format, before being able to load
    # the default config !

    #
    # Figure out which workflow to use.  This will influence the default output
    # format -> fragment_renderer_name, which in turn will change which default
    # set of config we will import.
    #
    workflow_name = (
        flm_run_info['workflow']
        or run_config.get('flm', {}).get('default_workflow', None)
        or 'templatebasedworkflow'
    )
    _, WorkflowClass = resource_accessor.import_class(
        workflow_name,
        default_prefix='flm.main.workflow',
        default_classnames=['RenderWorkflowClass'],
        flm_run_info=flm_run_info
    )

    #
    # Determine the fragment_renderer_name
    #

    flm_run_info['requested_outputformat'] = \
        flm_run_info['outputformat'] or run_config.get('flm', {}).get('default_format', None)

    fragment_renderer_name = WorkflowClass.get_fragment_renderer_name(
        flm_run_info['requested_outputformat'],
        flm_run_info,
        run_config
    )
    if not fragment_renderer_name:
        raise ValueError("Could not determine output format (fragment renderer name)")

    flm_run_info['fragment_renderer_name'] = fragment_renderer_name

    #
    # Now we know the workflow and the fragment_renderer_name, we can set up the
    # full configuration.
    #
    merge_default_configs = list(default_configs or [])

    workflow_default_main_config = \
        WorkflowClass.get_default_main_config(flm_run_info, run_config)
    if workflow_default_main_config is not None and workflow_default_main_config:
        merge_default_configs.extend([workflow_default_main_config])

    if add_builtin_default_configs:
        merge_default_configs.extend([
            _builtin_default_config['_byformat'].get(fragment_renderer_name, {}),
            _builtin_default_config['_base'],
        ])

    override_config = {}
    if flm_run_info.get('force_block_level', None) is not None:
        override_config = {
            'flm': {
                'parsing': {
                    'force_block_level': flm_run_info['force_block_level']
                }
            }
        }

    merge_configs = [
        override_config,
        run_config,
        *merge_default_configs
    ]
    # make a deep copy of everything so we can modify configs
    merge_configs = [
        copy.deepcopy(x)
        for x in merge_configs
    ]

    logger.debug('Merging configurations.  At this point, merge_configs = %s',
                 ",\n    ".join([f"{repr(m)}" for m in merge_configs]))

    # pull out feature-related config, don't merge these yet because we want to
    # pull in the defaults first.  See load_features()
    features_merge_configs = []
    for c in merge_configs:
        feature_merge_configs = {}
        flmconfig = c.get('flm', {})
        if flmconfig and flmconfig.get('features', None):
            for featurename, featureconfig in flmconfig['features'].items():
                if featurename.startswith('$'):
                    raise ValueError(
                        f"FIXME: presets not yet supported immediately inside "
                        f"‘features:’ config, got {featurename}"
                    )
                feature_merge_configs[featurename] = featureconfig
                if featureconfig is None or featureconfig is False:
                    c['flm']['features'][featurename] = False
                else:
                    c['flm']['features'][featurename] = True

        features_merge_configs.append(feature_merge_configs)

    # pull out workflow-related config, don't merge these yet because we want to
    # pull in the defaults first.
    workflows_merge_configs = []
    for c in merge_configs:
        workflow_merge_configs = {}
        flmconfig = c.get('flm', {})
        if flmconfig and flmconfig.get('workflow_config', None):
            for workflowname, workflowconfig in flmconfig['workflow_config'].items():
                if workflowname.startswith('$'):
                    raise ValueError(
                        f"FIXME: $-instruction presets in YAML not yet supported immediately inside "
                        f"‘workflow_config:’ config, got {workflowname}"
                    )
                workflow_merge_configs[workflowname] = workflowconfig
                if workflowconfig is None or workflowconfig is False:
                    c['flm']['workflow_config'][workflowname] = False
                else:
                    c['flm']['workflow_config'][workflowname] = True

        workflows_merge_configs.append(workflow_merge_configs)

    # merge the base config !
    config = configmerger.recursive_assign_defaults(merge_configs)

    flm_run_info['main_config'] = config

    logger.debug("Merged config (w/o workflow/feature configs) = %s",
                 abbrev_value_str(config, maxstrlen=512) )
    
    # validate the overall config structure
    validate_config_for_schema(
        'flm',
        _global_config_schema,
        config.get('flm', {}),
    )

    #
    # Set up the correct output directory (temporary directory, if applicable)
    #
    requires_temporary_directory_output = WorkflowClass.requires_temporary_directory_output(
        flm_run_info,
        run_config,
    )
    if requires_temporary_directory_output:
        use_temporary_directory_output = TemporaryDirectory()
        flm_run_info['output_cwd'] = use_temporary_directory_output.name
    else:
        use_temporary_directory_output = None

    #
    # Set up the correct template path
    #
    def _add_template_path(path):
        # TODO: maybe at some point in the future, actually resolve the path
        # as an URL ...
        if path.startswith('pkg:'):
            tmodname = path[len('pkg:'):]
            _, pkg_get_template_path = resource_accessor.import_class(
                tmodname,
                default_classnames=['get_template_path'],
                flm_run_info=flm_run_info
            )
            this_template_path = pkg_get_template_path()
        else:
            this_template_path = path
        resource_accessor.template_path.append(this_template_path)

    config_template_path = config.get('flm', {}).get('template_path', [])
    if config_template_path:
        for path in config_template_path:
            _add_template_path(path)
    flmrun_template_path = flm_run_info['add_template_path']
    if flmrun_template_path:
        if isinstance(flmrun_template_path, str):
            flmrun_template_path = flmrun_template_path.split(os.pathsep)
        for path in flmrun_template_path:
            _add_template_path(path)

    #
    # Set up the fragment renderer
    #
    _, fragment_renderer_information = resource_accessor.import_class(
        fragment_renderer_name,
        default_prefix='flm.fragmentrenderer',
        default_classnames=['FragmentRendererInformation'],
        flm_run_info=flm_run_info,
    )
    FragmentRendererClass = fragment_renderer_information.FragmentRendererClass

    # fragment_renderer properties from config
    fragment_renderer_config = config['flm']['renderer'].get(fragment_renderer_name, {})

    validate_config_for_class_typed_attributes(
        _join_config_path(['flm', 'renderer', fragment_renderer_name]),
        FragmentRendererClass,
        fragment_renderer_config,
    )

    fragment_renderer = FragmentRendererClass(
        config=fragment_renderer_config
    )

    #
    # Set up the workflow
    #

    workflow_config_merge_configs = [
        c.get(workflow_name, {})
        for c in workflows_merge_configs
    ]
    workflow_own_default_config = \
        WorkflowClass.get_workflow_default_config(flm_run_info, config)
    if workflow_own_default_config:
        workflow_config_merge_configs.extend([workflow_own_default_config])

    logger.debug("Workflow config: merging configs for ‘%s’: %r",
                 workflow_name, workflow_config_merge_configs)

    workflow_config = configmerger.recursive_assign_defaults(workflow_config_merge_configs)

    logger.debug("Loading workflow ‘%s’ with config = %s", workflow_name,
                 abbrev_value_str(workflow_config, maxstrlen=512))

    # validate the workflow config
    validate_config_for_tp(
        _join_config_path(['flm', 'workflow_config', workflow_name]),
        WorkflowClass.TypeWorkflowConfigDict,
        workflow_config,
    )

    workflow = WorkflowClass(
        workflow_config,
        flm_run_info,
        fragment_renderer_information,
        fragment_renderer,
    )

    
    #
    # Set up the environment: parsing state and features
    #

    parsing_config = config.get('flm', {}).get('parsing', {})

    validate_config_for_fn_kwargs(
        'flm.parsing',
        flmenvironment.standard_parsing_state,
        parsing_config,
    )
    parsing_state = flmenvironment.standard_parsing_state(**parsing_config)
    logger.debug("parsing_state = %r", parsing_state)

    features, feature_configs = load_features(features_merge_configs, flm_run_info)
    logger.debug("features = %r", features)

    environment = flmenvironment.make_standard_environment(
        parsing_state=parsing_state,
        features=features,
    )

    return WorkflowEnvironmentInformation(
        environment=environment,
        config=config,
        feature_configs=feature_configs,
        flm_run_info=flm_run_info,
        workflow=workflow,
        fragment_renderer_name=fragment_renderer_name,
        use_temporary_directory_output=use_temporary_directory_output,
    )




class Run:
    def __init__(self, flm_content,
                 *,
                 flm_run_info,
                 run_config,
                 inline_configs=None, # gets merged into run_config, overriding settings there
                 default_configs=None,
                 add_builtin_default_configs=True):
        super().__init__()


        self.flm_content = flm_content
        self.flm_run_info = flm_run_info
        self.run_config = None
        self.run_config_initial = run_config
        self.inline_configs = inline_configs
        self.default_configs = default_configs
        self.add_builtin_default_configs = add_builtin_default_configs

        # before anything else, merge in any run-config overrides
        # (inline_config) into run_config:
        if self.inline_configs is None:
            self.run_config = dict(self.run_config_initial)
        else:
            self.inline_configs = [cfg for cfg in self.inline_configs if cfg is not None]
            if len(self.inline_configs) == 0:
                self.run_config = dict(self.run_config_initial)
            else:
                self.run_config = configmerger.recursive_assign_defaults([
                    *self.inline_configs,
                    self.run_config_initial
                ])
        run_config = self.run_config
        logger.debug("Using run_config=%r", run_config)

        resource_accessor = flm_run_info['resource_accessor']

        wenv = load_workflow_environment(
            flm_run_info=flm_run_info,
            run_config=run_config,
            default_configs=default_configs,
            add_builtin_default_configs=add_builtin_default_configs
        )

        environment = wenv.environment
        config = wenv.config
        workflow = wenv.workflow
        fragment_renderer_name = wenv.fragment_renderer_name

        #
        # Prepare document metadata
        #
        doc_metadata = configmerger.recursive_assign_defaults([
            {
                '_flm_config': config['flm'],
                '_flm_workflow': workflow,
                '_flm_run_info': flm_run_info,
            },
            flm_run_info.get('metadata', {}),
            { k: v for (k,v) in config.items() if k != 'flm' }
        ])

        #
        # Find any "child" documents (CONTENT PARTS) and compile them, too.
        #
        content_parts_infos = {
            'parts': [],
            'by_type': {},
            # 'type_flm_spec_infos': {},
        }
        if 'content_parts' in config:
            for content_part_info in config['content_parts']:
                if 'input' not in content_part_info:
                    raise ValueError("Expected 'input:' in each entry in 'content_parts:' list")
                in_input_fname = content_part_info['input']
                if in_input_fname is not None:
                    in_input_content = resource_accessor.read_file(
                        flm_run_info.get('cwd', None),
                        in_input_fname,
                        'content_part',
                        flm_run_info
                    )
                    # parse content/frontmatter and keep line number offset
                    in_frontmatter_metadata, in_flm_content, in_line_number_offset = \
                        parse_frontmatter_content_linenumberoffset(in_input_content)
                else:
                    in_input_content = None
                    in_frontmatter_metadata = None
                    in_flm_content = ''
                    in_line_number_offset = 0

                in_metadata = configmerger.recursive_assign_defaults([
                    in_frontmatter_metadata or {},
                    content_part_info.get('metadata', {}),
                ])

                in_type = None
                if 'type' in content_part_info:
                    in_type = content_part_info['type']
                    in_label = content_part_info.get('label', None)
                    in_metadata_title = in_metadata.get(
                        'title',
                        '[part title not specified in included FLM file front matter]'
                    )

                    head_flm_content = (
                        '\\' + str(in_type) + '{' + in_metadata_title + '}'
                    )
                    if in_label:
                        head_flm_content += '\\label{' + str(in_label) + '}'
                    head_flm_content += '\n'

                    in_flm_content = head_flm_content + in_flm_content
                    in_line_number_offset -= head_flm_content.count('\n')

                in_input_lineno_colno_offsets = {
                    'line_number_offset': in_line_number_offset,
                }

                logger.debug('Document part FLM with auto-generated part type header:\n%s',
                             in_flm_content)

                cpinfo = dict(content_part_info)
                cpinfo['input_source'] = in_input_fname
                cpinfo['flm_content'] = in_flm_content
                cpinfo['metadata'] = in_metadata
                cpinfo['frontmatter_metadata'] = in_frontmatter_metadata
                cpinfo['input_lineno_colno_offsets'] = in_input_lineno_colno_offsets

                content_parts_infos['parts'].append( cpinfo )
                if in_type:
                    if in_type not in content_parts_infos['by_type']:
                        content_parts_infos['by_type'][in_type] = []
                        # mspec = environment.latex_context.get_macro_spec(
                        #     in_type, raise_if_not_found=True
                        # )
                        # content_parts_infos['type_flm_spec_infos'][in_type] = mspec

                    content_parts_infos['by_type'][in_type].append( cpinfo )

        self.content_parts_infos = content_parts_infos
        self.doc_metadata = doc_metadata
        self.resource_accessor = resource_accessor
        self.wenv = wenv


    def cleanup(self):
        self.wenv.cleanup()


    def run(self):

        flm_content = self.flm_content
        flm_run_info = self.flm_run_info
        run_config = self.run_config
        default_configs = self.default_configs
        add_builtin_default_configs = self.add_builtin_default_configs

        content_parts_infos = self.content_parts_infos
        doc_metadata = self.doc_metadata
        resource_accessor = self.resource_accessor
        wenv = self.wenv

        environment = wenv.environment
        config = wenv.config
        workflow = wenv.workflow
        fragment_renderer_name = wenv.fragment_renderer_name

        #
        # Set up the fragment (MAIN fragment in case of content-chapters)
        #
        silent = True # we'll report errors ourselves
        if logging.getLogger('flm').isEnabledFor(logging.DEBUG):
            # verbose logging is enabled, so don't be silent
            silent = False


        #
        # Compile main document fragment
        #

        what = flm_run_info.get('input_source', None)
        fragment = environment.make_fragment(
            flm_content,
            #is_block_level is already set in parsing_state
            silent=silent,
            input_lineno_colno_offsets=flm_run_info.get('input_lineno_colno_offsets', {}),
            what=what,
            resource_info=ResourceInfo(
                # resource_info.source_path is always relative to the document
                # root folder.
                source_path=doc_metadata.get('filepath', {}).get('basename', None)
            ),
        )

        #
        # Compile document fragments
        #

        document_parts_fragments = []
        for cpinfo in content_parts_infos['parts']:

            if cpinfo['flm_content'] is not None:
                in_fragment = environment.make_fragment(
                    cpinfo['flm_content'],
                    silent=silent,
                    input_lineno_colno_offsets=cpinfo['input_lineno_colno_offsets'],
                    what=f"Document Part ‘{cpinfo['input_source']}’",
                    resource_info=ResourceInfo(
                        source_path=cpinfo['input_source']
                    ),
                )
            else:
                in_fragment = None

            cpinfo['fragment'] = in_fragment

            if in_fragment is not None:
                document_parts_fragments.append(in_fragment)


        #
        # Build the document, with the rendering function from the workflow.
        #
        doc = environment.make_document(
            lambda render_context:
                workflow.render_document_fragment_callback(
                    fragment, render_context,
                    content_parts_infos=content_parts_infos,
                ),
            metadata=doc_metadata,
        )

        doc.document_fragments = [ fragment ]



        #
        # Allow features prime access to the document and the fragment, in case they
        # want to scan stuff (e.g., for citations)
        #
        for feature_name, feature_document_manager in doc.feature_document_managers:
            if hasattr(feature_document_manager, 'flm_main_scan_fragment'):
                feature_document_manager.flm_main_scan_fragment(
                    fragment,
                    document_parts_fragments=document_parts_fragments,
                    flm_run_info=self.flm_run_info
                )


        #
        # Render the document according to the workflow
        #

        result = workflow.render_document(doc, content_parts_infos=content_parts_infos)


        #
        # Prepare some information about the rendering & its result
        #
        result_info = {
            'environment': environment,
            'fragment_renderer_name': fragment_renderer_name,
            #'fragment_renderer': fragment_renderer, # use workflow.fragment_renderer
            'workflow': workflow,
            'binary_output': workflow.binary_output,
            'content_parts_infos': content_parts_infos,
            'document_parts_fragments': document_parts_fragments,
        }

        #
        # Done!
        #
        return result, result_info

    def get_config_json_schema(self):

        # Use all loaded feature classes and all workflow classes that appear in
        # workflow_config of the given config

        flm_run_info = self.flm_run_info
        resource_accessor = flm_run_info['resource_accessor']

        def _extract_cls_load_name(
                cls,
                default_prefix,
                default_classnames,
                flm_run_info=flm_run_info,
            ):
            # see if we can import by module name only
            modname = cls.__module__.removeprefix(default_prefix+'.')
            try:
                _, clsimport = resource_accessor.import_class(
                    modname,
                    default_prefix=default_prefix,
                    default_classnames=default_classnames,
                    flm_run_info=flm_run_info
                )
                if clsimport is cls:
                    # it's the same class!
                    return modname
            except (ValueError,ImportError):
                pass
            return cls.__qualname__.removeprefix(default_prefix+'.')

        feature_classes = dict([
            (_extract_cls_load_name(f.__class__, default_prefix='flm.feature', default_classnames=['FeatureClass']),
             f.__class__)
            for f in self.wenv.environment.features
        ])

        import pkgutil


        # include all known workflows
        from . import workflow as workflow_parent_module
        workflow_submodules = pkgutil.iter_modules(workflow_parent_module.__path__)
        logger.debug('Discovered builtin workflow submodules: %r', workflow_submodules)
        workflow_names = set([ m.name for m in workflow_submodules ])
        for wname in self.wenv.config.get('flm', {}).get('workflow_config', {}).keys():
            workflow_names.add(wname)

        workflow_classes = {}
        for wname in workflow_names:
            if wname == '_base':
                continue
            try:
                _, WorkflowClass = resource_accessor.import_class(
                    wname,
                    default_prefix='flm.main.workflow',
                    default_classnames=['RenderWorkflowClass'],
                    flm_run_info=flm_run_info
                )
                workflow_classes[wname] = WorkflowClass
            except Exception as e:
                logger.warning(f"Not including JSON schema for workflow ‘{wname}’, module load failed: {e}")
                continue

        # include all known renderers
        from .. import fragmentrenderer as renderer_parent_module
        renderer_submodules = pkgutil.iter_modules(renderer_parent_module.__path__)
        logger.debug('Discovered builtin renderer submodules: %r', renderer_submodules)
        renderer_names = set([ m.name for m in renderer_submodules] )
        for rname in self.wenv.config.get('flm', {}).get('renderer', {}).keys():
            renderer_names.add(rname)
        
        renderer_classes = {}
        for rname in renderer_names:
            if rname == '_base':
                continue
            try:
                _, fragment_renderer_information = resource_accessor.import_class(
                    rname,
                    default_prefix='flm.fragmentrenderer',
                    default_classnames=['FragmentRendererInformation'],
                    flm_run_info=flm_run_info,
                )
                FragmentRendererClass = fragment_renderer_information.FragmentRendererClass
                renderer_classes[rname] = FragmentRendererClass
            except Exception as e:
                logger.warning(f"Not including JSON schema for renderer ‘{rname}’, module load failed: {e}")
                continue

        # call module-level function
        return get_config_json_schema(
            feature_classes=feature_classes,
            workflow_classes=workflow_classes,
            renderer_classes=renderer_classes,
        )


def run(*args, **kwargs):
    R = Run(*args, **kwargs)
    try:
        run_result = R.run()
    finally:
        R.cleanup()
    return run_result



