# Generate a JavaScript version of FLM via Transcrypt

Set up your poetry environment with the 'buildjslib' dependency group:

    > poetry install --with buildjslib

Check out the pylatexenc sources. (If you've done this in the past, you might
want to do `git pull` in that directory to get the latest version.)

    > git clone https://github.com/phfaist/pylatexenc pylatexenc-src
    > # alternatively, make sure we have the lastest version:
    > (cd pylatexenc-src && git pull)

Generate the relevant JavaScript FLM Sources by running the build script:

    > poetry run python generate_flm_js.py --pylatexenc-src-dir=./pylatexenc-src/

