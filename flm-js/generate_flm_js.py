import os
import os.path
import sys
import argparse

import shutil

import logging
logger = logging.getLogger('generate_flm_js')


flm_src_dir = os.path.normpath(os.path.join(os.path.dirname(__file__), '..'))

def run_main():
    parser = argparse.ArgumentParser()

    parser.add_argument('--flm-js-output-dir', action='store',
                        default='flm-js',
                        help="Folder where to output generated JavaScript FLM sources")

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
                        help="Also compile the FLM tests into a separate folder ./test-flm-js")
    parser.add_argument('--test-flm-js-output-dir', action='store',
                        default='test-flm-js',
                        help="Folder where to output generated JavaScript FLM test sources. "
                        "The main entry point for the tests will be the script 'runtests.js'")

    args = parser.parse_args()

    logging.basicConfig(level=logging.DEBUG)

    if args.delete_target_dir:
        if os.path.exists(args.flm_js_output_dir):
            shutil.rmtree(args.flm_js_output_dir)
        if args.compile_tests:
            if os.path.exists(args.test_flm_js_output_dir):
                shutil.rmtree(args.test_flm_js_output_dir)

    os.makedirs(args.preprocess_lib_output_dir, exist_ok=True)

    if os.path.exists(args.flm_js_output_dir):
        raise RuntimeError(f"Target destination ‘{args.flm_js_output_dir}’ already exists. "
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
            'FLM_SRC_DIR': flm_src_dir,
        }
    )

    override_enabled_features = None
    if args.enable_debug:
        override_enabled_features['keep_logger_debug'] = True

    # preprocess both pylatexenc & flm libraries to prepare them for Transcrypt -->
    genutils.preprocess_pylatexenc_lib(override_enabled_features=override_enabled_features)
    genutils.preprocess_lib('preprocesslib-flm.config.yaml',
                            override_enabled_features=override_enabled_features)
    if args.compile_tests:
        genutils.preprocess_lib('preprocesslib-tests.config.yaml')

    # run Transcrypt FLM lib now -->
    genutils.run_transcrypt(
        'import_flm_modules.py',
        output_dir=args.flm_js_output_dir,
    )
    # final tweaks to finalize the JS package
    genutils.finalize_transcrypt_package(
        args.flm_js_output_dir,
        package_name='flm-js',
        package_version='0.0.1',
        package_description=\
            'Automatically transliterated Javascript version of the FLM sources'
    )


    if args.compile_tests:

        # Generate the test runner script
        runtests_py = genutils.generate_runtests_script(
            os.path.join(flm_src_dir, 'test'),
        )

        # Transcrypt it
        genutils.run_transcrypt(
            runtests_py,
            add_import_paths=[
                os.path.join(args.preprocess_lib_output_dir, 'test')
            ],
            output_dir=args.test_flm_js_output_dir,
        )
        genutils.finalize_transcrypt_package(
            args.test_flm_js_output_dir,
            package_name='test-flm-js',
        )

        logger.info("Compiled the tests. To run them, try ‘node {}/runtests.js’"
                    .format(args.test_flm_js_output_dir))

    logger.info(f"Done!")







if __name__ == '__main__':
    run_main()
