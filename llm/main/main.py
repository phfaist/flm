import sys
import os.path
import fileinput
import re
import json

import logging
logger = logging.getLogger(__name__)

import frontmatter
import yaml


from .importclass import import_class
from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from . import run


class ResourceAccessor(run.ResourceAccessorBase):

    template_path = [
        None, # we interpret `None` as relative to the LLM source file's folder
        os.path.realpath(os.path.join(os.path.dirname(__file__), 'templates')),
    ]

    def file_exists(self, fpath, fname, ftype):
        fullpath = self.get_full_path(fpath, fname, ftype)
        return os.path.exists(fullpath)

    def read_file(self, fpath, fname, ftype):
        fullpath = self.get_full_path(fpath, fname, ftype)
        with open(fullpath, encoding='utf-8') as f:
            return f.read()

    def get_full_path(self, fpath, fname, ftype):
        if not fpath:
            return fname
        return os.path.join(fpath, fname)

    def import_class(self, fullname, **kwargs):
        return import_class(fullname, **kwargs)






def main(**kwargs):

    arg_format = kwargs.get('format', None)
    arg_workflow = kwargs.get('workflow', None)
    arg_template = kwargs.get('template', None)
    arg_force_block_level = kwargs.get('force_block_level', None)
    arg_files = kwargs.get('files', None)
    arg_llm_content = kwargs.get('llm_content', None)
    arg_config = kwargs.get('config', None)
    arg_output = kwargs.get('output', None)
    arg_suppress_final_newline = kwargs.get('suppress_final_newline', None)


    logger.debug("Format is %r", arg_format)

    # Get the LLM content

    input_content = ''
    dirname = None
    basename = None
    jobname = 'unknown-jobname'
    jobnameext = None
    if arg_llm_content is not None:
        if arg_files is not None and len(arg_files):
            raise ValueError(
                "You cannot specify both FILEs and --llm-content options. "
                "Type `llm --help` for more information."
            )
        input_content = arg_llm_content
    elif arg_files is None:
        # doesn't happen on the command line because arg_files is always a
        # list, possibly an empty one.  This trap is only useful for
        # programmatic invocation of runmain()
        raise ValueError(
            r"No input specified. Please use llm_content or specify input files."
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
                           "LLM content lines.")
        for line in fileinput.input(files=arg_files, encoding='utf-8'):
            input_content += line

    frontmatter_metadata, llm_content = frontmatter.parse(input_content)

    # compute line number offset (it doesn't look like I can grab this from the
    # `frontmatter` module's result :/
    rx_frontmatter = re.compile(r"^-{3,}\s*$\s*", re.MULTILINE) # \s also matches newline
    m = rx_frontmatter.search(input_content) # top separator
    if m is not None:
        m = rx_frontmatter.search(input_content, m.end()) # below the front matter
    line_number_offset = 0
    if m is not None:
        line_number_offset = input_content[:m.end()].count('\n') + 1

    # load config & defaults

    config_file = arg_config
    orig_config = {}
    if isinstance(config_file, str) and config_file:
        # parse a YAML file
        with open(config_file) as f:
            orig_config = yaml.safe_load(f)
    elif isinstance(config_file, dict):
        orig_config = config_file
    else:
        # see if there's a llmconfig.(yaml|yml) in the current directory, and
        # load that one if applicable.
        if os.path.exists('llmconfig.yaml'):
            with open('llmconfig.yaml') as f:
                orig_config = yaml.safe_load(f)
        elif os.path.exists('llmconfig.yml'):
            with open('llmconfig.yml') as f:
                orig_config = yaml.safe_load(f)
        

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

    llm_run_info = {
        'resource_accessor': ResourceAccessor(),
        'outputformat': arg_format,
        'workflow': arg_workflow,
        'template': arg_template,
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
        llm_content,
        llm_run_info=llm_run_info,
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