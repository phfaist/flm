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



transcrypt_options = (
    '--dassert --dext --gen --tconv --sform --kwargs --keycheck --xreex '
    '--opov ' # let's hope we can get away w/o this one sometime in the future....
    '--nomin --build --anno --parent .none -u .auto'.split()
)

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

    parser.add_argument('--preprocessed-temp-dir', action='store', default='pp-tmp',
                        help="Temporary folder in which to write intermediate, "
                        "preprocessed sources to be fed into Transcrypt")

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

    os.makedirs(args.preprocessed_temp_dir, exist_ok=True)

    if os.path.exists(args.llm_js_output_dir):
        raise RuntimeError(f"Target destination ‘{args.llm_js_output_dir}’ already exists. "
                           f"Please remove it first.")

    logger.debug(f"Using pylatexenc dir ‘{args.pylatexenc_src_dir}’")
    if not os.path.isdir(args.pylatexenc_src_dir):
        raise RuntimeError(f"Please provide the location of pylatexenc sources with "
                           f"the --pylatexenc-src-dir option, see README")

    # set up environment for subprocesses
    env = {}
    env.update(os.environ)
    env['PYLATEXENC_SRC_DIR'] = args.pylatexenc_src_dir
    env['PREPROCESSED_TEMP_DIR'] = args.preprocessed_temp_dir

    def run_cmd(cmd):
        logger.info(f"Running {cmd}")
        subprocess.run(cmd, env=env, check=True, stdout=None, stderr=None,)

    python = sys.executable
    preprocess_lib_py = os.path.join(args.pylatexenc_src_dir, 'tools/preprocess_lib.py')


    # preprocess both pylatexenc & llm libraries to prepare them for Transcrypt -->
    run_cmd([python, preprocess_lib_py, 'preprocesslib-pylatexenc.config.yaml'])
    run_cmd([python, preprocess_lib_py, 'preprocesslib-llm.config.yaml'])
    if args.compile_tests:
        run_cmd([python, preprocess_lib_py, 'preprocesslib-tests.config.yaml'])

    # run Transcrypt LLM lib now -->
    transcrypt_import_path = "$".join([ # combine with '$' separator, see transcrypt --help
        args.preprocessed_temp_dir,
        os.path.join(args.pylatexenc_src_dir, 'js-transcrypt/libpatches'),
    ])
    run_cmd([python, '-m', 'transcrypt',
             'import_llm_modules.py', *transcrypt_options,
             '-xp', transcrypt_import_path,
             '-od', args.llm_js_output_dir,])

    # final tweaks to finalize the JS package
    finalize_js_package(args.llm_js_output_dir)


    if args.compile_tests:

        # Generate the test runner script
        generated_runtests_script_fname = \
            os.path.join(args.preprocessed_temp_dir, 'runtests.py')
        logger.info(f"Creating test runner script ...")
        with open( generated_runtests_script_fname, 'w' ) as fw:
            fw.write(r"""
import logging
logging.basicConfig(level=logging.DEBUG)

import unittest
""")
            testmodnames = []
            for testpyname in  os.listdir('../test'):
                m = re.match('^(?P<testmodname>test_.*)[.]py$', testpyname)
                if m is not None:
                    testmodnames.append( m.group('testmodname') )
            
            fw.write("\n".join([ f"import {mod}" for mod in testmodnames ]) + "\n")
            fw.write(r"""my_test_modules = [ """ + "\n".join([
                f"{x}, " for x in testmodnames
            ]) + " ]\n")
            fw.write(r"""
print("Instantiating tests...")
my_test_classes = []
for module in my_test_modules:
    for membername in dir(module):
        if membername.startswith('Test'):
            cls = getattr(module, membername)
            #print("Class is ", cls.__name__)
            instance = cls()
            #print("Instance is ", instance)
            my_test_classes.append( [membername, instance] )

print("About to run all tests...")

unittest.do_run(my_test_classes)
""")

        # Also compile version with tests; run Transcrypt LLM lib now -->
        transcrypt_import_path = "$".join([ # combine with '$' separator, see transcrypt --help
            args.preprocessed_temp_dir,
            os.path.join(args.preprocessed_temp_dir, 'test'),
            os.path.join(args.pylatexenc_src_dir, 'js-transcrypt/libpatches'),
        ])
        run_cmd([python, '-m', 'transcrypt',
                 generated_runtests_script_fname,
                 *transcrypt_options,
                 '-xp', transcrypt_import_path,
                 '-od', os.path.abspath(args.test_llm_js_output_dir),])

        # finalize the package
        finalize_js_package(args.test_llm_js_output_dir)

        logger.info("Compiled the tests. To run them, try ‘node {}/runtests.js’"
                    .format(args.test_llm_js_output_dir))

    logger.info(f"Done!")






def finalize_js_package(js_package_dir):
    # create package.json -->
    logger.info(f"Creating package.json ...")
    with open( os.path.join(js_package_dir, 'package.json'), 'w' ) as fw:
        json.dump({
            "name": "llm-js",
            "version": "0.0.1",
            "description": "Automatically translated javascript version of the LLM sources",
            "type": "module",
            #"private": True,
        }, fw)

    # Simple interface for e.g. kwargs in transcrypt's runtime
    logger.info(f"Installing shortcuts to python runtime tricks ...")
    with open( os.path.join(js_package_dir, 'py.js'), 'w' ) as fw:
        fw.write(r"""
import {__kwargtrans__, repr} from "./org.transcrypt.__runtime__.js";
const $$kw = __kwargtrans__;
export { $$kw, repr };
""")

    #
    # Apply some patches to Transcrypt's generated JS runtime package
    #
    logger.info(f"Applying some patches to the JS runtime ...")

    runtimejs_fname = os.path.join(js_package_dir, 'org.transcrypt.__runtime__.js')
    with open( runtimejs_fname ) as f:
        runtimejs = f.read()

    # patch up the implementation of the __pop__() function.  To this effect
    # we'll simply rename that function to some dummy value and append a new
    # definition at the end of the file.
    runtimejs = re.sub(r'function\s+__pop__\s*\(',
                       'function TRANSCRYPT_IMPL__pop__ (',
                       runtimejs)

    # apply additional JS patches immediately when any llm-related package is loaded -->
    # (append to Transcrypt's runtime js definitions source file)

    with open( os.path.join(os.path.dirname(__file__), 'transcrypt_runtime_patches.js') ) as f:
        transcrypt_runtime_patches = f.read()

    runtimejs += "\n\n" + transcrypt_runtime_patches

    with open( runtimejs_fname, 'w' ) as f:
        f.write(runtimejs)




if __name__ == '__main__':
    run_main()
