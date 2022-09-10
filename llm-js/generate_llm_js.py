import os
import os.path
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
                        "at the beginning of the script instead of throwing an error.")

    args = parser.parse_args()

    logging.basicConfig(level=logging.DEBUG)

    if args.delete_target_dir:
        if os.path.exists(args.llm_js_output_dir):
            shutil.rmtree(args.llm_js_output_dir)

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

    def run_cmd(cmd):
        logger.info(f"Running {cmd}")
        subprocess.run(cmd, env=env, check=True, stdout=None, stderr=None,)

    python = sys.executable
    preprocess_lib_py = os.path.join(args.pylatexenc_src_dir, 'tools/preprocess_lib.py')


    # preprocess both pylatexenc & llm libraries to prepare them for Transcrypt -->
    run_cmd([python, preprocess_lib_py, 'preprocesslib-pylatexenc.config.yaml'])
    run_cmd([python, preprocess_lib_py, 'preprocesslib-llm.config.yaml'])

    # run Transcrypt now -->
    run_cmd([python, '-m', 'transcrypt',
             'import_llm_modules.py', *transcrypt_options,
             '-xp', os.path.join(args.pylatexenc_src_dir, 'js-transcrypt/libpatches'),
             '-od', args.llm_js_output_dir,])

    # final tweaks.

    # create package.json -->
    logger.info(f"Creating package.json ...")
    with open( os.path.join(args.llm_js_output_dir, 'package.json'), 'w' ) as fw:
        json.dump({
            "name": "llm-js",
            "version": "0.0.1",
            "description": "Automatically translated javascript version of the LLM sources",
            "type": "module",
            "private": True
        }, fw)

    # Simple interface for e.g. kwargs in transcrypt's runtime
    logger.info(f"Installing shortcuts to python runtime tricks ...")
    with open( os.path.join(args.llm_js_output_dir, 'py.js'), 'w' ) as fw:
        fw.write(r"""
import {__kwargtrans__, repr} from "./org.transcrypt.__runtime__.js";
const $$kw = __kwargtrans__;
export { $$kw, repr };
""")

    # apply JS patches immediately when any llm-related package is loaded -->
    # (append to Transcrypt's runtime js definitions source file)
    logger.info(f"Installing JS patches ...")
    with open( os.path.join(args.llm_js_output_dir,
                            'org.transcrypt.__runtime__.js'), 'a' ) as fw:
        fw.write(r"""
/** BEGIN UGLY HACK **/
import {custom_apply_patches} from "./customjspatches.js";
custom_apply_patches();
/** END UGLY HACK **/
""")
    
    logger.info(f"Done!")



if __name__ == '__main__':
    run_main()
