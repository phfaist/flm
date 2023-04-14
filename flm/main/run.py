import copy
import os # os.pathsep
import os.path
import logging
logger = logging.getLogger(__name__)

from typing import Any, Optional
from collections.abc import Mapping
from dataclasses import dataclass

import yaml

from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from ._util import abbrev_value_str

from flm import flmenvironment

# ---

class ResourceAccessorBase:

    template_exts = ['', '.yaml', '.yml', '.json']
    
    template_path = [
        None
    ]

    def get_template_info_file_name(self, template_prefix, template_name, flm_run_info):

        cwd = flm_run_info.get('cwd', None)

        for tpath in self.template_path:

            if tpath is None:
                tpath = cwd # might still be `None`

            for text in self.template_exts:
                tfullname = os.path.join(template_prefix, f"{template_name}{text}")
                if self.file_exists(tpath, tfullname, 'template_info'):
                    return tpath, tfullname

        raise ValueError(
            f"Template not found: ‘{template_name}’.  "
            f"Template path is = {repr(self.template_path)}"
        )

    def import_class(self, fullname, *, default_classnames=None, default_prefix=None):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def read_file(self, fpath, fname, ftype):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def file_exists(self, fpath, fname, ftype):
        raise RuntimeError("Must be reimplemented by subclasses!")

    def dir_exists(self, fpath, fname, ftype):
        raise RuntimeError("Must be reimplemented by subclasses!")
        


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

    for featurename, featureconfig in features_onoff.items():

        if featureconfig is None or featureconfig is False:
            continue
        if featureconfig is True:
            featureconfig = {}

        _, FeatureClass = resource_accessor.import_class(
            featurename,
            default_prefix='flm.feature',
            default_classnames=['FeatureClass'],
        )

        # re-merge the config fully from the initial merge configs, so that we
        # make sure we honor $defaults etc. properly
        
        feature_merge_configs = [
            c.get(featurename, {})
            for c in features_merge_configs
        ]

        if hasattr(FeatureClass, 'feature_default_config'):
            feature_merge_configs.append( FeatureClass.feature_default_config or {} )

        featureconfig = configmerger.recursive_assign_defaults(feature_merge_configs)

        logger.debug("Instantiating feature ‘%s’ with config = %s", featurename,
                     abbrev_value_str(featureconfig, maxstrlen=512) )

        features.append( FeatureClass(**featureconfig) )

    return features




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


_builtin_default_config_yaml = os.path.join(os.path.dirname(__file__), 'default_config.yaml')
with open(_builtin_default_config_yaml, encoding='utf-8') as f:
    _builtin_default_config = yaml.safe_load(f)




@dataclass
class WorkflowEnvironmentInformation:

    environment : Optional[Any] = None

    config : Optional[Mapping] = None

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
        default_classnames=['RenderWorkflowClass']
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

    override_config = { 'flm': { 'parsing': {} } }
    if flm_run_info.get('force_block_level', None) is not None:
        override_config['flm']['parsing']['force_block_level'] = \
            flm_run_info['force_block_level']

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

    # pull out feature-related config, don't merge these yet because we want to
    # pull in the defaults first.  See load_features()
    features_merge_configs = []
    for c in merge_configs:
        feature_merge_configs = {}
        flmconfig = c.get('flm', {})
        if flmconfig and flmconfig.get('features', None):
            for featurename, featureconfig in flmconfig['features'].items():
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
                default_classnames=['get_template_path']
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

    features = load_features(features_merge_configs, flm_run_info)
    logger.debug("features = %r", features)

    environment = flmenvironment.make_standard_environment(
        parsing_state=parsing_state,
        features=features,
    )

    return WorkflowEnvironmentInformation(
        environment=environment,
        config=config,
        flm_run_info=flm_run_info,
        workflow=workflow,
        fragment_renderer_name=fragment_renderer_name
    )






def run(flm_content,
        *,
        flm_run_info,
        run_config,
        default_configs=None,
        add_builtin_default_configs=True):

    wenv = load_workflow_environment(
        flm_run_info=flm_run_info,
        run_config=run_config,
        default_configs=default_configs,
        add_builtin_default_configs=add_builtin_default_configs
    )

    environment = wenv.environment
    config = wenv.config
    flm_run_info = wenv.flm_run_info
    workflow = wenv.workflow
    fragment_renderer_name = wenv.fragment_renderer_name


    #
    # Set up the fragment
    #
    fragment = environment.make_fragment(
        flm_content,
        #is_block_level is already set in parsing_state
        silent=True, # we'll report errors ourselves
        input_lineno_colno_offsets=flm_run_info.get('input_lineno_colno_offsets', {}),
        what=flm_run_info.get('input_source', None)
    )

    #
    # Prepare document metadata
    #
    doc_metadata = configmerger.recursive_assign_defaults([
        {
            '_flm_config': config['flm'],
            '_flm_workflow': workflow,
        },
        flm_run_info.get('metadata', {}),
        { k: v for (k,v) in config.items() if k != 'flm' }
    ])
    
    #
    # Build the document, with the rendering function from the workflow.
    #
    doc = environment.make_document(
        lambda render_context:
            workflow.render_document_fragment_callback(fragment, render_context),
        metadata=doc_metadata
    )
    
    
    #
    # Allow features prime access to the document and the fragment, in case they
    # want to scan stuff (e.g., for citations)
    #
    for feature_name, feature_document_manager in doc.feature_document_managers:
        if hasattr(feature_document_manager, 'flm_main_scan_fragment'):
            feature_document_manager.flm_main_scan_fragment(fragment)


    #
    # Render the document according to the workflow
    #

    result = workflow.render_document(doc)


    #
    # Prepare some information about the rendering & its result
    #
    result_info = {
        'environment': environment,
        'fragment_renderer_name': fragment_renderer_name,
        #'fragment_renderer': fragment_renderer, # use workflow.fragment_renderer
        'workflow': workflow,
        'binary_output': workflow.binary_output,
    }

    #
    # Done!
    #
    return result, result_info




