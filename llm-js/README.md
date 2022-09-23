# Generate a JavaScript version of LLM via Transcrypt

Set up your poetry environment with extras:

    > poetry install -E buildjslib

Check out the pylatexenc sources. (If you've done this in the past, you might
want to do `git pull` in that directory to get the latest version.)

    > git clone https://github.com/phfaist/pylatexenc pylatexenc-src -b devel
    > # alternatively, make sure we have the lastest version:
    > (cd pylatexenc-src && git pull)

Generate the relevant JavaScript LLM Sources by running the build script:

    > poetry run python generate_llm_js.py --pylatexenc-src-dir=./pylatexenc-src/


Internals:

The generator script actually runs the following commands (this was some time ago,
now the script was updated to something slightly fancier and more streamlined):

    export PYLATEXENC_SRC_DIR=../PATH/TO/pylatexenc
    #ln -s $PYLATEXENC_SRC_DIR/js-transcrypt/pylatexenc . ### NO LONGER NEEDED
    ln -s $PYLATEXENC_SRC_DIR/js-transcrypt/libpatches .
    poetry run python $PYLATEXENC_SRC_DIR/tools/preprocess_lib.py preprocesslib-pylatexenc.config.yaml
    poetry run python $PYLATEXENC_SRC_DIR/tools/preprocess_lib.py preprocesslib-llm.config.yaml
    poetry run transcrypt import_llm_modules.py --dassert --dext --gen --tconv --sform --kwargs --keycheck --opov --xreex --nomin --build --anno --parent .none -u .auto -xp 'libpatches' -od llm-js
    cp template-package.json llm-js/package.json
    echo -e '\n/** HACK **/ import {custom_apply_patches} from "./customjspatches.js"; custom_apply_patches();' >>llm-js/org.transcrypt.__runtime__.js
    

