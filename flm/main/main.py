import sys
import os.path
import fileinput
import json

import logging
logger = logging.getLogger(__name__)

import yaml

import watchfiles


from .importclass import import_class as _import_class
from .configmerger import ConfigMerger
configmerger = ConfigMerger()

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

    def read_file(self, fpath, fname, ftype, flm_run_info):
        fullpath = self.get_full_path(fpath, fname, ftype, flm_run_info)
        with open(fullpath, encoding='utf-8') as f:
            content = f.read()
        return content

    def get_full_path(self, fpath, fname, ftype, flm_run_info):
        if not fpath:
            return fname
        return os.path.join(fpath, fname)

    def import_class(self, fullname, *, flm_run_info, **kwargs):
        return _import_class(fullname, **kwargs)





def load_external_config(arg_config, dirname):
    config_file = arg_config
    if isinstance(config_file, str) and config_file:
        # parse a YAML file
        with open(config_file, encoding='utf-8') as f:
            data = yaml.safe_load(f)
            data['$_cwd'] = os.path.dirname(config_file)
            return data
    elif isinstance(config_file, dict):
        return config_file
    else:
        # see if there's a flmconfig.(yaml|yml) in the same directory as the
        # input file, and load that one if applicable.
        cfgfnamesbase = [ 'flmconfig.yaml', 'flmconfig.yml' ]
        for cfgfnamebase in cfgfnamesbase:
            cfgfname = os.path.join(dirname or '', cfgfnamebase)
            if os.path.exists(cfgfname):
                with open(cfgfname, encoding='utf-8') as f:
                    logger.debug(f"Found config file {cfgfname}, loading it.")
                    data = yaml.safe_load(f)
                    data['$_cwd'] = dirname
                    return data
        
    return {}




def main_watch(**kwargs):

    arg_files = kwargs.get('files', None)
    arg_output = kwargs.get('output', None)

    if arg_output is None or arg_output == '-':
        raise ValueError(
            "Please provide an output file (-o OUTPUT_FILE) when enablig watch mode"
        )

    main(**kwargs)
    
    logger.info('Watching input files, hit Interrupt (Ctrl+C) to quit.')

    for changes in watchfiles.watch(*arg_files, raise_interrupt=False):
        logger.info('Input file(s) changed: %r', changes)
        
        try:
            main(**kwargs)
        except Exception as e:
            logger.error("Error recompiling document! %s", e, exc_info=e)
            
        
    logger.info('Okay, quitting now.')



def main(**kwargs):

    arg_format = kwargs.get('format', None)
    arg_workflow = kwargs.get('workflow', None)
    arg_template = kwargs.get('template', None)
    arg_template_path = kwargs.get('template_path', None)
    arg_force_block_level = kwargs.get('force_block_level', None)
    arg_files = kwargs.get('files', None)
    arg_flm_content = kwargs.get('flm_content', None)
    arg_config = kwargs.get('config', None)
    arg_output = kwargs.get('output', None)
    arg_suppress_final_newline = kwargs.get('suppress_final_newline', None)


    logger.debug("Format is %r", arg_format)

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


    frontmatter_metadata, flm_content, line_number_offset = \
        run.parse_frontmatter_content_linenumberoffset(input_content)

    # load config & defaults

    orig_config = load_external_config(arg_config, dirname)

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
        'input_source': arg_files,
        'input_lineno_colno_offsets': {
            'line_number_offset': line_number_offset,
        },
        'metadata': doc_metadata,
    }

    #
    # Run!
    #
    result, result_info = run.run(
        flm_content,
        flm_run_info=flm_run_info,
        run_config=run_config,
        default_configs=[ orig_config, ]
    )

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

    return





# ------------------------------------------------------------------------------



class _TrivialContextManager:
    def __init__(self, value):
        super().__init__()
        self.value = value

    def __enter__(self):
        return self.value

    def __exit__(*args):
        pass
