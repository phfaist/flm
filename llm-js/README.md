# Generate a JavaScript version of LLM via Transcrypt

Commands to run are:

    export PYLATEXENC_SRC_DIR=../PATH/TO/pylatexenc
    ln -s $PYLATEXENC_SRC_DIR/js-transcrypt/pylatexenc .
    ln -s $PYLATEXENC_SRC_DIR/js-transcrypt/libpatches .
    poetry run python $PYLATEXENC_SRC_DIR/tools/preprocess_lib.py preprocesslib.config.yaml
    poetry run transcrypt import_llm_modules.py --dassert --dext --gen --tconv --sform --kwargs --keycheck --opov --xreex --nomin --build --anno --parent .none -u .auto -xp 'libpatches' -od llm-js
    cp template-package.json llm-js/package.json
    echo -e '\n/** HACK **/ import {custom_apply_patches} from "./customjspatches.js"; custom_apply_patches();' >>llm-js/org.transcrypt.__runtime__.js
    

