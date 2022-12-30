import os
import os.path
import re
import sys
import argparse
import json

import shutil
import subprocess

import logging
logger = logging.getLogger('generate_llm_js')


llm_src_dir = os.path.normpath(os.path.join(os.path.dirname(__file__), '..'))

def run_main():
    parser = argparse.ArgumentParser()

    parser.add_argument('--llm-js-output-dir', action='store',
                        default='llm-js',
                        help="Folder where to output generated JavaScript LLM sources")

    parser.add_argument('--pylatexenc-src-dir', action='store',
                        default=os.path.join( os.path.dirname(__file__), 'pylatexenc-src'),
                        help="Path to the pylatexenc3 sources, preferably the latest version.")

    parser.add_argument('--delete-target-dir', action='store_true', default=False,
                        help="With this option, the target directory is removed if it exists "
                        "at the beginning of the script instead of throwing an error.  Will "
                        "also remove the tests target directory if --compile-tests is given.")

    parser.add_argument('--preprocess-lib-output-dir', action='store', default='pp-tmp',
                        help="Temporary folder in which to write intermediate, "
                        "preprocessed sources to be fed into Transcrypt")

    parser.add_argument('--enable-debug', action='store_true', default=False,
                        help="Generate logging debug() message calls instead of "
                        " removing them")

    parser.add_argument('--compile-tests', action='store_true', default=False,
                        help="Also compile the LLM tests into a separate folder ./test-llm-js")
    parser.add_argument('--test-llm-js-output-dir', action='store',
                        default='test-llm-js',
                        help="Folder where to output generated JavaScript LLM test sources. "
                        "The main entry point for the tests will be the script 'runtests.js'")

    args = parser.parse_args()

    logging.basicConfig(level=logging.DEBUG)

    if args.delete_target_dir:
        if os.path.exists(args.llm_js_output_dir):
            shutil.rmtree(args.llm_js_output_dir)
        if args.compile_tests:
            if os.path.exists(args.test_llm_js_output_dir):
                shutil.rmtree(args.test_llm_js_output_dir)

    os.makedirs(args.preprocess_lib_output_dir, exist_ok=True)

    if os.path.exists(args.llm_js_output_dir):
        raise RuntimeError(f"Target destination ‘{args.llm_js_output_dir}’ already exists. "
                           f"Please remove it first.")

    logger.debug(f"Using pylatexenc dir ‘{args.pylatexenc_src_dir}’")
    if not os.path.isdir(args.pylatexenc_src_dir):
        raise RuntimeError(f"Please provide the location of pylatexenc sources with "
                           f"the --pylatexenc-src-dir option, see README")

    # pick up pylatexenc's generation script tool

    pylatexenc_tools_dir = os.path.join(args.pylatexenc_src_dir, 'tools')
    logger.info(f"Using pylatexenc_tools_dir = {pylatexenc_tools_dir!r}")
    sys.path.insert(0, pylatexenc_tools_dir)

    import utils_transcrypt_generate_js

    genutils = utils_transcrypt_generate_js.GenUtils(
        pylatexenc_src_dir=args.pylatexenc_src_dir,
        preprocess_lib_output_dir=args.preprocess_lib_output_dir,
        env={
            'LLM_SRC_DIR': llm_src_dir,
        }
    )

    override_enabled_features = None
    if args.enable_debug:
        override_enabled_features['keep_logger_debug'] = True

    # preprocess both pylatexenc & llm libraries to prepare them for Transcrypt -->
    genutils.preprocess_pylatexenc_lib(override_enabled_features=override_enabled_features)
    genutils.preprocess_lib('preprocesslib-llm.config.yaml',
                            override_enabled_features=override_enabled_features)
    if args.compile_tests:
        genutils.preprocess_lib('preprocesslib-tests.config.yaml')

    # run Transcrypt LLM lib now -->
    genutils.run_transcrypt(
        'import_llm_modules.py',
        output_dir=args.llm_js_output_dir,
    )
    # final tweaks to finalize the JS package
    genutils.finalize_transcrypt_package(
        args.llm_js_output_dir,
        package_name='llm-js',
        package_version='0.0.1',
        package_description=\
            'Automatically transliterated Javascript version of the LLM sources'
    )


    if args.compile_tests:

        # Generate the test runner script
        runtests_js = genutils.generate_runtests_script(
            os.path.join(llm_src_dir, 'test'),
        )

        # Transcrypt it
        genutils.run_transcrypt(
            runtests_js,
            add_import_paths=[
                os.path.join(args.preprocess_lib_output_dir, 'test')
            ],
            output_dir=args.test_llm_js_output_dir,
        )
        genutils.finalize_transcrypt_package(
            args.test_llm_js_output_dir,
            package_name='test-llm-js',
        )

        logger.info("Compiled the tests. To run them, try ‘node {}/runtests.js’"
                    .format(args.test_llm_js_output_dir))

    logger.info(f"Done!")







if __name__ == '__main__':
    run_main()
