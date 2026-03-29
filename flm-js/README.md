# Generate a JavaScript version of FLM via Transcrypt

You can build a JS version of the core FLM library!  For instance,
such a version is bundled and included in the
[ZooDb Project](https://github.com/phfaist/zoodb), a JS package for
building zoos such as the [Error Correction Zoo](https://errorcorrectionzoo.org/).

Depending on what you're trying to achieve, the simplest approach might
be to use `ZooDb` directly.  The `ZooDb` package exposes several low-level
features and mechanisms of FLM.  If this is not quite what you need,
read on below.

## How to compile a JS version of FLM

The JS version of FLM's core modules is obtained by using the great tool
[Transcrypt](https://www.transcrypt.org/).

Set up your poetry environment with the 'buildjslib' dependency group:

    > poetry install --with buildjslib

Check out the pylatexenc sources. (If you've done this in the past, you might
want to do `git pull` in that directory to get the latest version.)

    > git clone https://github.com/phfaist/pylatexenc pylatexenc-src
    > # alternatively, make sure we have the lastest version:
    > (cd pylatexenc-src && git pull)

Generate the relevant JavaScript FLM Sources by running the build script:

    > poetry run python generate_flm_js.py --pylatexenc-src-dir=./pylatexenc-src/

Result: A new folder `flm-js/` with a flat layout of JS modules (with qualified
names such as `flm.feature.math.js`) that you can import in your code.
Note that some special conventions are needed
for `isinstance()`, writing subclasses, for passing keyword arguments, etc. —
Check the ZooDb sources to get a hang of it.
[Get started here.](https://github.com/phfaist/zoodb/blob/main/src/zooflm/_environment.js)

## Advanced: compile & run tests

To compile and run the tests, you can use the following command (WILL OVERWRITE
TARGET DIRECTORY `test-flm-js`):

    > poetry run python generate_flm_js.py --pylatexenc-src-dir=./pylatexenc-src/ --delete-target-dir --compile-tests
    > node test-flm-js/runtests.js
