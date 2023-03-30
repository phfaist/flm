import os.path
import logging
logger = logging.getLogger(__name__)

import yaml

from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from llm import llmenvironment

# ---

class ResourceAccessorBase:

    template_exts = ['', '.yaml', '.yml', '.json']
    
    template_path = [ None ]

    def get_template_info_file_name(self, template_prefix, template_name, llm_run_info):

        cwd = llm_run_info.get('cwd', None)

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
        


# ------------------


# ..........
# llm_run_info = {
#     'resource_accessor': .... # instance of ResourceAccessorBase subclass

#     'outputformat': .....
#     'workflow': ......
#     'force_block_level': None|true|false

#     'cwd': ..... # input CWD
#     'input_lineno_colno_offsets': ..... # passed on to llmfragment, adjust line/col numbers
#     'metadata': ..... # to be merged into the document's metadata. Can
#                       # include information about the LLM source, etc.
# }



# ---


def load_features(config, llm_run_info):

    features_config = config['llm']['features']

    resource_accessor = llm_run_info['resource_accessor']

    features = []

    for featurename, featureconfig in features_config.items():

        if featureconfig is None or featureconfig is False:
            continue
        if featureconfig is True:
            featureconfig = {}
        
        _, FeatureClass = resource_accessor.import_class(
            featurename,
            default_prefix='llm.feature',
            default_classnames=['FeatureClass'],
        )

        if hasattr(FeatureClass, 'default_config'):
            defaultconfig = dict(FeatureClass.feature_default_config)
            if defaultconfig:
                featureconfig = configmerger.recursive_assign_defaults([
                    featureconfig,
                    defaultconfig
                ])

        features.append( FeatureClass(**featureconfig) )

    return features




#
# config
# ======
#
# llm:
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


_builtin_default_config_yaml = os.path.join(os.path.dirname(__file__), 'default_config.yaml')
with open(_builtin_default_config_yaml, encoding='utf-8') as f:
    _builtin_default_config = yaml.safe_load(f)


def run(llm_content,
        *,
        llm_run_info,
        run_config,
        default_configs=None,
        add_builtin_default_configs=True):

    resource_accessor = llm_run_info['resource_accessor']

    # Set up the workflow to get the output format, before being able to load
    # the default config !

    #
    # Figure out which workflow to use.  This will influence the default output
    # format -> fragment_renderer_name, which in turn will change which default
    # set of config we will import.
    #
    workflow_name = (
        llm_run_info['workflow']
        or run_config.get('llm', {}).get('default_workflow', None)
        or 'templatebasedworkflow'
    )
    _, WorkflowClass = resource_accessor.import_class(
        workflow_name,
        default_prefix='llm.main.workflow',
        default_classnames=['RenderWorkflowClass']
    )

    #
    # Determine the fragment_renderer_name
    #
    fragment_renderer_name = (
        llm_run_info['outputformat']
        or WorkflowClass.get_default_fragment_renderer(llm_run_info, run_config)
        or run_config.get('llm', {}).get('default_format', {})
    )
    llm_run_info['fragment_renderer_name'] = fragment_renderer_name

    #
    # Now we know the fragment_renderer_name, set up the full configuration.
    #
    merge_default_configs = list(default_configs or [])
    if add_builtin_default_configs:
        merge_default_configs.extend([
            _builtin_default_config['_byformat'].get(fragment_renderer_name, {}),
            _builtin_default_config['_base'],
        ])

    override_config = { 'llm': { 'parsing': {} } }
    if llm_run_info.get('template', None) is not None:
        override_config['llm']['template'] = {
            fragment_renderer_name: {
                'name': llm_run_info['template']
            }
        }
    if llm_run_info.get('force_block_level', None) is not None:
        override_config['llm']['parsing']['force_block_level'] = \
            llm_run_info['force_block_level']

    config = configmerger.recursive_assign_defaults([
        override_config,
        run_config,
        *merge_default_configs
    ])

    llm_run_info['main_config'] = config

    #
    # Set up the fragment renderer
    #
    _, fragment_renderer_information = resource_accessor.import_class(
        fragment_renderer_name,
        default_prefix='llm.fragmentrenderer',
        default_classnames=['FragmentRendererInformation'],
    )
    FragmentRendererClass = fragment_renderer_information.FragmentRendererClass

    # fragment_renderer properties from config
    fragment_renderer_config = config['llm']['renderer'].get(fragment_renderer_name, {})

    fragment_renderer = FragmentRendererClass(
        config=fragment_renderer_config
    )

    #
    # Set up the workflow
    #

    workflow_config = configmerger.recursive_assign_defaults([
        config['llm']['workflow'].get(workflow_name, {}),
        WorkflowClass.get_workflow_default_config(llm_run_info, config),
    ])
    workflow = WorkflowClass(
        workflow_config,
        llm_run_info,
        fragment_renderer_information,
        fragment_renderer
    )

    
    #
    # Set up the environment: parsing state and features
    #

    parsing_state = llmenvironment.standard_parsing_state(**config['llm']['parsing'])
    logger.debug("parsing_state = %r", parsing_state)

    features = load_features(config, llm_run_info)
    logger.debug("features = %r", features)

    environment = llmenvironment.make_standard_environment(
        parsing_state=parsing_state,
        features=features,
    )

    #
    # Set up the fragment
    #
    fragment = environment.make_fragment(
        llm_content,
        #is_block_level is already set in parsing_state
        silent=True, # we'll report errors ourselves
        input_lineno_colno_offsets=llm_run_info.get('input_lineno_colno_offsets', {}),
    )

    #
    # Prepare document metadata
    #
    doc_metadata = configmerger.recursive_assign_defaults([
        {
            '_config_llm': config['llm'],
        },
        llm_run_info.get('metadata', {}),
        { k: v for (k,v) in config.items() if k != 'llm' }
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
        if hasattr(feature_document_manager, 'llm_main_scan_fragment'):
            feature_document_manager.llm_main_scan_fragment(fragment)


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


