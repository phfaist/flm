import re
import copy
import os # os.pathsep
import os.path
import logging
logger = logging.getLogger(__name__)

from typing import Any, Optional
from collections.abc import Mapping
from dataclasses import dataclass

import frontmatter
import yaml

from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from ._util import abbrev_value_str

from flm import flmenvironment

# ---

class ResourceAccessorBase:
    r"""
    Interface to access templates, import feature/workflow class instances,
    and read FS files.  See main subclass :py:class:`flm.main.ResourceAccessor`.
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

        raise ValueError(
            f"Template not found: ‘{template_name}’.  "
            f"Template path is = {repr(self.template_path)}"
        )

    def find_in_search_paths(self, search_paths, fname, ftype, flm_run_info):

        cwd = flm_run_info.get('cwd', None)

        for search_path in search_paths:

            if cwd is not None:
                search_path = os.path.join(cwd, search_path)

            if self.file_exists(search_path, fname, ftype, flm_run_info):
                return search_path, fname

        raise ValueError(
            f"File not found: ‘{fname}’. Search path was = {repr(search_paths)}"
        )

    def import_class(self, fullname, *, default_classnames=None, default_prefix=None,
                     flm_run_info=None):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def read_file(self, fpath, fname, ftype, flm_run_info, binary=False):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def open_file_object_context(self, fpath, fname, ftype, flm_run_info, binary=False):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def file_exists(self, fpath, fname, ftype, flm_run_info):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def dir_exists(self, fpath, fname, ftype, flm_run_info):
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
    def __init__(self, source_path):
        super().__init__()
        self.source_path = source_path

        if source_path is not None:
            self._source_dirname = os.path.dirname(self.source_path)
        else:
            self._source_dirname = None

    def get_source_directory(self):
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

#     'cwd': ..... # input CWD
#     'input_lineno_colno_offsets': ..... # passed on to flmfragment, adjust line/col numbers
#     'metadata': ..... # to be merged into the document's metadata. Can
#                       # include information about the FLM source, etc.
# }



# ---


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

        _, FeatureClass = resource_accessor.import_class(
            featurename,
            default_prefix='flm.feature',
            default_classnames=['FeatureClass'],
            flm_run_info=flm_run_info
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


def load_workflow_environment(*,
                              flm_run_info,
                              run_config,
                              default_configs=None,
                              add_builtin_default_configs=True):

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

    # logger.debug('DEBUG! At this point, merge_configs = %s',
    #              ",\n    ".join([f"{repr(m)}" for m in merge_configs]))

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
                        f"FIXME: presets not yet supported immediately inside "
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

    workflow_config = configmerger.recursive_assign_defaults(workflow_config_merge_configs)

    logger.debug("Loading workflow ‘%s’ with config = %s", workflow_name,
                 abbrev_value_str(workflow_config, maxstrlen=512))

    workflow = WorkflowClass(
        workflow_config,
        flm_run_info,
        fragment_renderer_information,
        fragment_renderer
    )

    
    #
    # Set up the environment: parsing state and features
    #

    parsing_state = flmenvironment.standard_parsing_state(**config['flm']['parsing'])
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
    )






def run(flm_content,
        *,
        flm_run_info,
        run_config,
        default_configs=None,
        add_builtin_default_configs=True):

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
    # Set up the fragment (MAIN fragment in case of content-chapters)
    #
    silent = True # we'll report errors ourselves
    if logging.getLogger('flm').isEnabledFor(logging.DEBUG):
        # verbose logging is enabled, so don't be silent
        silent = False

    what = flm_run_info.get('input_source', None)
    fragment = environment.make_fragment(
        flm_content,
        #is_block_level is already set in parsing_state
        silent=silent,
        input_lineno_colno_offsets=flm_run_info.get('input_lineno_colno_offsets', {}),
        what=what,
        resource_info=ResourceInfo(
            source_path=flm_run_info.get('input_source', None)
        ),
    )


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
    document_parts_fragments = []
    if 'content_parts' in config:
        for content_part_info in config['content_parts']:
            if 'input' not in content_part_info:
                raise ValueError("Expected 'input:' in each entry in 'content_parts:' list")
            in_input_fname = content_part_info['input']
            in_input_content = resource_accessor.read_file(
                flm_run_info.get('cwd', None),
                in_input_fname,
                'content_part',
                flm_run_info
            )

            # parse content/frontmatter and keep line number offset
            in_frontmatter_metadata, in_flm_content, in_line_number_offset = \
                parse_frontmatter_content_linenumberoffset(in_input_content)


            in_type = None
            if 'type' in content_part_info:
                in_type = content_part_info['type']
                in_label = content_part_info.get('label', None)
                in_frontmatter_title = (in_frontmatter_metadata or {}).get(
                    'title',
                    '[part title not specified in included FLM file front matter]'
                )

                head_flm_content = (
                    '\\' + str(in_type) + '{' + in_frontmatter_title + '}'
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

            in_fragment = environment.make_fragment(
                in_flm_content,
                silent=silent,
                input_lineno_colno_offsets=in_input_lineno_colno_offsets,
                what=f"Document Part ‘{in_input_fname}’",
                resource_info=ResourceInfo(
                    source_path=in_input_fname
                ),
            )

            document_parts_fragments.append(in_fragment)

            cpinfo = dict(content_part_info)
            cpinfo['input_source'] = in_input_fname
            cpinfo['fragment'] = in_fragment
            cpinfo['flm_content'] = in_flm_content
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




