import sys
import os.path
import fileinput
import json

import logging
logger = logging.getLogger(__name__)

import yaml



from .importclass import import_class as _import_class
from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from ._util import ReprValueFallbackJsonEncoder

from . import run


class ResourceAccessor(run.ResourceAccessorBase):

    template_path = [
        None, # we interpret `None` as relative to the FLM source file's folder
        os.path.realpath(os.path.join(os.path.dirname(__file__), 'templates')),
    ]

    def file_exists(self, fpath, fname, ftype, flm_run_info):
        fullpath = self.get_full_path(fpath, fname, ftype, flm_run_info)
        return os.path.exists(fullpath) and os.path.isfile(fullpath)

    def dir_exists(self, fpath, fname, ftype, flm_run_info):
        fullpath = self.get_full_path(fpath, fname, ftype, flm_run_info)
        return os.path.exists(fullpath) and os.path.isdir(fullpath)

    def _open_r(self, fullpath, binary):
        if binary:
            return open(fullpath, 'rb')
        return open(fullpath, 'r', encoding='utf-8')
        
    def read_file(self, fpath, fname, ftype, flm_run_info, binary=False):
        fullpath = self.get_full_path(fpath, fname, ftype, flm_run_info)
        with self._open_r(fullpath, binary) as f:
            content = f.read()
        return content

    def open_file_object_context(self, fpath, fname, ftype, flm_run_info, binary=False):
        fullpath = self.get_full_path(fpath, fname, ftype, flm_run_info)
        return self._open_r(fullpath, binary)

    def get_full_path(self, fpath, fname, ftype, flm_run_info):
        if not fpath:
            return fname
        return os.path.join(fpath, fname)

    def import_class(self, fullname, *, flm_run_info=None, **kwargs):
        return _import_class(fullname, **kwargs)





def load_external_configs(dirname, *, arg_config=None, arg_format=None, arg_workflow=None):

    load_config_files = []

    # figure out which config files to load.
    if isinstance(arg_config, dict):
        load_config_files = [ arg_config ]
    else:
        # we're going to load one or more config files.
        fnameconfigbase = "flmconfig"
        fnametryexts = (".yaml", ".yml",) # only the FIRST EXISTING EXTENSION is read.
        if isinstance(arg_config, str):
            fnamepath, fnameconfigtail = os.path.split(arg_config)
            fnameconfigbase, fnametrysingleext = os.path.splitext(fnameconfigtail)
            fnametryexts = ( fnametrysingleext, )
        # attempt to load different variants of the config file, and use/merge them all.
        # [ flmconfig+WORKFLOW.FORMAT.yaml, flmconfig+WORKFLOW.yaml,
        #   flmconfig.FORMAT.yaml, flmconfig.yaml ].
        fnametryworkflowparts = ("",)
        if arg_workflow:
            fnametryworkflowparts = (f"+{arg_workflow}", "",)
        fnametryformatparts = ("",)
        if arg_format:
            fnametryworkflowparts = (f".{arg_format}", "",)
        for workflowpart in fnametryworkflowparts:
            for formatpart in fnametryformatparts:
                for ext in fnametryexts:
                    tryfname = f"{fnameconfigbase}{workflowpart}{formatpart}{ext}"
                    if os.path.exists(tryfname):
                        load_config_files.append(tryfname)
                        # Now, break the innermost for loop (over extensions).
                        # Once a pattern with a valid extension is found, don't
                        # try any other extensions and proceed to the next
                        # workflow/format pattern. -->
                        break

    logger.debug("Identified config files to load: %r", load_config_files)

    loaded_config_datas = []
    for config_file in load_config_files:
        # parse a YAML file
        if isinstance(config_file, dict):
            data = config_file
        else:
            with open(config_file, encoding='utf-8') as f:
                logger.info(f"Loading flm config from {config_file}")
                data = yaml.safe_load(f)
                data['$_cwd'] = os.path.dirname(config_file)
        loaded_config_datas.append( data )

    if len(loaded_config_datas) == 0:
        loaded_config_datas = [ {} ]

    return loaded_config_datas



def _process_arg_inline_configs(arg_inline_configs):
    if arg_inline_configs is None:
        return None
    if isinstance(arg_inline_configs, str):
        # remember, JSON is valid YAML
        arg_inline_configs = yaml.safe_load(arg_inline_configs)
    if isinstance(arg_inline_configs, dict):
        arg_inline_configs = [ arg_inline_configs ]
    arg_inline_configs = [
        (yaml.safe_load(cfg) if isinstance(cfg, str) else cfg)
        for cfg in arg_inline_configs
        if cfg is not None
    ]
    if len(arg_inline_configs) == 0:
        return None
    return arg_inline_configs



class Main:
    def __init__(self, **kwargs):
        super().__init__()

        self.kwargs = kwargs

        self.arg_format = kwargs.get('format', None)
        self.arg_workflow = kwargs.get('workflow', None)
        self.arg_template = kwargs.get('template', None)
        self.arg_template_path = kwargs.get('template_path', None)
        self.arg_force_block_level = kwargs.get('force_block_level', None)
        self.arg_files = kwargs.get('files', None)
        self.arg_flm_content = kwargs.get('flm_content', None)
        self.arg_config = kwargs.get('config', None)
        self.arg_output = kwargs.get('output', None)
        self.arg_suppress_final_newline = kwargs.get('suppress_final_newline', None)

        # these options are called inline_config, not inline_configs, because
        # they translate to a --inline-config command-line option that can be
        # specified more than once.
        self.arg_inline_configs = kwargs.get('inline_config', None)
        self.arg_inline_default_configs = kwargs.get('inline_default_config', None)

        self._no_default_stdin = kwargs.get('_no_default_stdin', None)

        # ---

        self.arg_inline_configs = _process_arg_inline_configs(self.arg_inline_configs)
        self.arg_inline_default_configs = \
            _process_arg_inline_configs(self.arg_inline_default_configs)

        arg_format = self.arg_format
        arg_workflow = self.arg_workflow
        arg_template = self.arg_template
        arg_template_path = self.arg_template_path
        arg_force_block_level = self.arg_force_block_level
        arg_files = self.arg_files
        arg_flm_content = self.arg_flm_content
        arg_config = self.arg_config
        arg_inline_configs = self.arg_inline_configs
        arg_inline_default_configs = self.arg_inline_default_configs
        arg_output = self.arg_output
        arg_suppress_final_newline = self.arg_suppress_final_newline

        logger.debug("Format is %r", self.arg_format)

        logger.debug("inline_configs is %r", self.arg_inline_configs)
        logger.debug("inline_default_configs is %r", self.arg_inline_default_configs)

        # Get the FLM content

        input_content = ''
        dirname = None
        basename = None
        jobname = 'unknown-jobname'
        jobnameext = None
        if arg_flm_content is not None:
            if arg_files is not None and len(arg_files):
                raise ValueError(
                    "You cannot specify both FILEs and --flm-content options. "
                    "Type `flm --help` for more information."
                )
            input_content = arg_flm_content
        elif arg_files is None:
            # doesn't happen on the command line because arg_files is always a
            # list, possibly an empty one.  This trap is only useful for
            # programmatic invocation of runmain()
            raise ValueError(
                r"No input specified. Please use flm_content or specify input files."
            )
        elif len(arg_files) == 0 and self._no_default_stdin:
            logger.debug("No files provided but not defaulting to stdin per internal instructions")
            input_content = ''
        else:
            if len(arg_files) >= 1 and arg_files[0] != '-':
                dirname, basename = os.path.split(arg_files[0])
                jobname, jobnameext = os.path.splitext(basename)
            if len(arg_files) >= 2:
                logger.warning("When multiple files are given, only the YAML front matter "
                               "for the first specified file is inspected.  The jobname is "
                               "set to the name of the first file.  Line numbers past the "
                               "end of the first file will refer to the total processed "
                               "FLM content lines.")
            for line in fileinput.input(files=arg_files, encoding='utf-8'):
                input_content += line

        output_dirname = dirname
        output_basename = None
        output_jobname = None
        output_jobnameext = None
        if arg_output is not None:
            if isinstance(arg_output, str) and arg_output != '-':
                output_dirname, output_basename = os.path.split(arg_output)
                output_jobname, output_jobnameext = os.path.splitext(basename)

        frontmatter_metadata, flm_content, line_number_offset = \
            run.parse_frontmatter_content_linenumberoffset(input_content)

        # load config & defaults

        orig_configs = load_external_configs(
            dirname,
            arg_config=arg_config,
            arg_format=arg_format,
            arg_workflow=arg_workflow,
        )
        if arg_inline_default_configs is not None:
            orig_configs = [
                # user flmconfig.yaml files
                *orig_configs,
                # --inline-default-config
                *arg_inline_default_configs,
                # (the actual feature & workflow defaults will be appended by run.Run)
            ]

        logger.debug("Input frontmatter_metadata is\n%s",
                     json.dumps(frontmatter_metadata,indent=4))

        doc_metadata = {
            'filepath': {
                'dirname': dirname,
                'basename': basename,
                'jobnameext': jobnameext,
            },
            'jobname': jobname,
        }

        self.input_content = input_content
        self.dirname = dirname
        self.basename = basename
        self.jobname = jobname
        self.jobnameext = jobnameext

        self.frontmatter_metadata = frontmatter_metadata
        self.flm_content = flm_content
        self.line_number_offset = line_number_offset
        self.orig_configs = orig_configs
        self.doc_metadata = doc_metadata


        run_config = frontmatter_metadata or {}
        run_config['$_cwd'] = dirname

        resource_accessor = ResourceAccessor()

        flm_run_info = {
            'resource_accessor': resource_accessor,
            'outputformat': arg_format,
            'workflow': arg_workflow,
            'template': arg_template or None, # or None will replace an empty string by None
            'add_template_path': arg_template_path,
            'force_block_level': arg_force_block_level,
            'cwd': dirname,
            'output_filepath': {
                'dirname': output_dirname,
                'basename': output_basename,
                'jobname': output_jobname,
                'jobnameext': output_jobnameext,
            },
            'output_cwd': output_dirname,
            'input_source': arg_files[0] if (arg_files and len(arg_files)) else None,
            'input_lineno_colno_offsets': {
                'line_number_offset': line_number_offset,
            },
            'metadata': doc_metadata,
        }

        self.run_config = run_config
        self.resource_accessor = resource_accessor
        self.flm_run_info = flm_run_info


    def make_run_object(self):
        r"""
        You can call this instead of run() if you want to get the Run()
        instance, instead of proceeding through the full run.

        Don't call this separately if you already call run().
        """

        arg_format = self.arg_format
        arg_workflow = self.arg_workflow
        arg_template = self.arg_template
        arg_template_path = self.arg_template_path
        arg_force_block_level = self.arg_force_block_level
        arg_files = self.arg_files
        arg_flm_content = self.arg_flm_content
        arg_config = self.arg_config
        arg_output = self.arg_output
        arg_suppress_final_newline = self.arg_suppress_final_newline

        return run.Run(
            self.flm_content,
            flm_run_info=self.flm_run_info,
            inline_configs=self.arg_inline_configs, # overrides run_config
            run_config=self.run_config,
            # arg_inline_default_configs are already included in self.orig_configs:
            default_configs=self.orig_configs,
        )

    def run(self, skip_write_return_result=False):

        arg_format = self.arg_format
        arg_workflow = self.arg_workflow
        arg_template = self.arg_template
        arg_template_path = self.arg_template_path
        arg_force_block_level = self.arg_force_block_level
        arg_files = self.arg_files
        arg_flm_content = self.arg_flm_content
        arg_config = self.arg_config
        arg_output = self.arg_output
        arg_suppress_final_newline = self.arg_suppress_final_newline

        run_object = self.make_run_object()

        try:

            #
            # Run!
            #
            result, result_info = run_object.run()

            if skip_write_return_result:
                return {
                    "result": result,
                    "result_info": result_info
                }
            
        finally:
            run_object.cleanup()

        binary_output = result_info['binary_output']

        #
        # Write to output
        #
        def open_context_fout():
            if not arg_output or arg_output == '-':
                stream = sys.stdout
                if binary_output:
                    stream = sys.stdout.buffer
                return _TrivialContextManager(stream)
            elif hasattr(arg_output, 'write'):
                # it's a file-like object, use it directly
                return _TrivialContextManager(arg_output)
            else:
                return open(arg_output, 'w' + ('b' if binary_output else ''))


        with open_context_fout() as fout:

            fout.write(result)

            if not binary_output and not arg_suppress_final_newline:
                fout.write("\n")

            if isinstance(arg_output, str) and arg_output != '-':
                logger.info('Output to ‘%s’', arg_output)

        main_run_info = {
            'flm_run_info': self.flm_run_info,
            'flm_content': self.flm_content,
            'run_config': self.run_config,
            'result': result,
            'result_info': result_info,
            'output': arg_output,
            'binary_output': binary_output,
        }

        return main_run_info



def main(**kwargs):
    a = Main(**kwargs)
    return a.run()


_acceptable_print_merged_config_keys = [
    'run',
    'full',
    'full-flm',
    'workflow',
    'template',
]
def main_print_merged_config(**kwargs):
    a = Main(**kwargs)
    run_object = a.make_run_object()
    # extract fully merged config
    print_merged_config = kwargs['print_merged_config']
    if print_merged_config == 'run':
        print(json.dumps(run_object.run_config, indent=4, cls=ReprValueFallbackJsonEncoder))
    elif print_merged_config == 'full':
        print(json.dumps(run_object.flm_run_info['main_config'],
                         indent=4, cls=ReprValueFallbackJsonEncoder))
    elif print_merged_config == 'full-flm':
        print(json.dumps(run_object.flm_run_info['main_config']['flm'],
                         indent=4, cls=ReprValueFallbackJsonEncoder))
    elif print_merged_config == 'workflow':
        print(json.dumps(run_object.wenv.workflow.config,
                         indent=4, cls=ReprValueFallbackJsonEncoder))
    elif print_merged_config == 'template':
        w = run_object.wenv.workflow
        use_output_format_name = w.get_use_output_format_name()
        use_template_name = w.get_use_template_name(use_output_format_name)
        print("# use_output_format_name =", json.dumps(use_output_format_name))
        print("# use_template_name =", json.dumps(use_template_name))
        if use_template_name is None or not use_template_name:
            # no template specified
            logger.warning("No template specified; no configuration available")
            return

        template_prefix, template_config_wdefaults = \
            w.get_merged_template_config_with_prefix(
                use_output_format_name,
                use_template_name
            )
        print("# template_prefix =", json.dumps(template_prefix))
        print(json.dumps(template_config_wdefaults,
                         indent=4, cls=ReprValueFallbackJsonEncoder))
    else:
        raise ValueError(
            f"Invalid --print-merged-config=‘{print_merged_config}’, "
            f"expected on of 'run', 'full', 'workflow', 'template'"
        )
    return None
main_print_merged_config.available_keys = _acceptable_print_merged_config_keys






def main_validate_config(*args, **kwargs):
    r"""
    Simply validate the config and emit logger warning errors for config validation failures.
    Return after the loading of workflow, features, etc. is complete.  Does not compile the
    FLM document.
    """
    a = Main(**kwargs)
    # constructing the main object validates all the object's config
    run_object = a.make_run_object()
    return



def main_print_config_json_schema(*args, _print_fn=print, **kwargs):
    r"""
    Output a JSON schema that FLM configurations should conform to.
    """
    a = Main(**kwargs, _no_default_stdin=True)
    run_object = a.make_run_object()
    schema = run_object.get_config_json_schema()
    _print_fn( json.dumps(schema) )
    return






# ------------------------------------------------------------------------------



class _TrivialContextManager:
    def __init__(self, value):
        super().__init__()
        self.value = value

    def __enter__(self):
        return self.value

    def __exit__(self, *args):
        pass
