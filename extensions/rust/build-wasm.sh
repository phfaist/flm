#!/usr/bin/env bash
# Builds the WebAssembly forms of the extensions shipped in `extensions/`. It writes one of
# two outputs.
#
#     extensions/rust/build-wasm.sh [--out DIR] [NAME ...]
#     extensions/rust/build-wasm.sh --collection FILE NAME ...
#
# The folder of one extension, for the `flm` program and any host that finds extensions by
# name on its search path. For each extension NAME written in Rust (every folder of this
# folder holding a `flmextension.yaml` when no name is given), the script runs
#
#     cargo rustc -p flm-extensions --no-default-features --features NAME --target wasm32-unknown-unknown --profile wasm-extension --crate-type cdylib
#
# from the root of the checkout: the crate `flm-extensions` with the extension's feature
# alone, built as a module that holds that one extension's code and no file. It writes the
# folder DIR/NAME/ holding the module, under the file name that the extension's manifest
# gives (its key `module`: `profile_pdf_latex.wasm`), and the extension's other files: every
# entry of its folder except `src/` and `tests/` (its code and its tests) and the names
# beginning with `.`, its `flmextension.yaml` and its `README.md` among them. DIR defaults
# to `target/wasm-extensions/` of the checkout. The folder DIR/NAME is an extension as the
# `flm` program finds it on its search path: `flm render --extensions-dir DIR …`.
#
# A collection, for a host program that loads the file itself and offers the extensions it
# holds (the `flm` program reads none). The script runs, once,
#
#     cargo rustc -p flm-extensions --no-default-features --features collection,NAME,... --target wasm32-unknown-unknown --profile wasm-extension --crate-type cdylib
#
# and writes the module as FILE: one module that holds the code of each NAME written in
# Rust and, in its custom section `flmextensions`, every file of each NAME. A NAME is any
# extension shipped in `extensions/`: of the folder `data/`, `js/` or `rust/`. The name of
# FILE ends in `.flmextensions.wasm`.
#
# Each output is built in a cargo run of its own, with the package `flm-extensions`
# selected alone: within one cargo run, `flm-ext` is compiled once, with one set of
# features, and a run that also builds the `flm` program would turn on `flm-ext`'s feature
# `native`, which leaves the module without its exports. The profile `wasm-extension` is
# declared in the workspace's `Cargo.toml` (the release profile, optimized for size,
# aborting on a panic). The cargo target folder is the one cargo uses for the checkout
# (`CARGO_TARGET_DIR` when it is set).
#
# Neither a cargo build nor the tests run this script, except the tests of the WebAssembly
# form in `flm/tests/pdf_latex.rs`, `flm/tests/tty.rs` and `flm/tests/math_techxt.rs`, and
# the test of a collection in `flm-host/tests/collection.rs`, gated on
# `FLM_TESTS_WASM_GUEST=1`.

set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
target=wasm32-unknown-unknown
profile=wasm-extension

usage() {
    echo "usage: extensions/rust/build-wasm.sh [--out DIR] [NAME ...]"
    echo "       extensions/rust/build-wasm.sh --collection FILE NAME ..."
    echo "Builds the WebAssembly forms of the extensions shipped in extensions/."
    echo "With --out or no option: the folder DIR/NAME/ of each extension NAME written in Rust (all of them"
    echo "  when no NAME is given; DIR: target/wasm-extensions/ of the checkout), holding its module and its files."
    echo "  For the flm program and any host that finds extensions by name: flm render --extensions-dir DIR ..."
    echo "With --collection: one module FILE, whose name ends in .flmextensions.wasm, holding every extension"
    echo "  NAME (of data/, js/ or rust/) with its files. For a host program that loads the file and offers its"
    echo "  extensions; the flm program reads none."
}

out=
# Set by the option `--collection`, whatever its value: the output is then a collection.
as_collection=
collection=
names=()
while [ $# -gt 0 ]; do
    case "$1" in
        --out | --collection)
            if [ $# -lt 2 ]; then
                echo "build-wasm.sh: missing value of the option ‘$1’" >&2
                exit 2
            fi
            if [ "$1" = --out ]; then out=$2; else as_collection=1 collection=$2; fi
            shift 2
            ;;
        --out=*)
            out=${1#--out=}
            shift
            ;;
        --collection=*)
            as_collection=1
            collection=${1#--collection=}
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            echo "build-wasm.sh: unknown option ‘$1’: the options are ‘--out’, ‘--collection’ and ‘--help’" >&2
            exit 2
            ;;
        *)
            names+=("$1")
            shift
            ;;
    esac
done

# A shipped extension of `extensions/`: a folder of `data/`, `js/` or `rust/` holding
# `flmextension.yaml`, or a file `NAME.yaml` of `data/` or `js/`. Its cargo feature has its
# name.
shipped() {
    local folder
    for folder in data js rust; do
        if [ -f "$root/extensions/$folder/$1/flmextension.yaml" ]; then
            return 0
        fi
    done
    for folder in data js; do
        if [ -f "$root/extensions/$folder/$1.yaml" ]; then
            return 0
        fi
    done
    return 1
}

if [ -n "$as_collection" ]; then
    if [ -n "$out" ]; then
        echo "build-wasm.sh: the option ‘--out’ may not be used with ‘--collection’" >&2
        exit 2
    fi
    case "$(basename "$collection")" in
        ?*.flmextensions.wasm) ;;
        *)
            echo "build-wasm.sh: invalid name of the collection file ‘${collection}’: it must end in ‘.flmextensions.wasm’" >&2
            exit 2
            ;;
    esac
    if [ ${#names[@]} -eq 0 ]; then
        echo "build-wasm.sh: missing extension names after ‘--collection ${collection}’" >&2
        exit 2
    fi
    for name in "${names[@]}"; do
        if ! shipped "$name"; then
            echo "build-wasm.sh: missing extension ‘${name}’ in the folders ‘data/’, ‘js/’ and ‘rust/’ of ‘$root/extensions’" >&2
            exit 2
        fi
    done
else
    out=${out:-"$root/target/wasm-extensions"}
    if [ ${#names[@]} -eq 0 ]; then
        for folder in "$here"/*/; do
            if [ -f "$folder/flmextension.yaml" ]; then
                names+=("$(basename "$folder")")
            fi
        done
    fi
    for name in "${names[@]}"; do
        if [ ! -f "$here/$name/flmextension.yaml" ]; then
            echo "build-wasm.sh: missing extension ‘${name}’ in the folder ‘${here}’" >&2
            exit 2
        fi
    done
fi

sysroot=$(cd "$root" && rustc --print sysroot)
if [ ! -d "$sysroot/lib/rustlib/$target" ]; then
    echo "build-wasm.sh: missing Rust target ‘${target}’; install it with ‘rustup target add ${target}’" >&2
    exit 1
fi

target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
if [ -z "$target_dir" ]; then
    echo "build-wasm.sh: missing target folder of the checkout ‘${root}’ in the output of ‘cargo metadata’" >&2
    exit 1
fi
built="$target_dir/$target/$profile/flm_extensions.wasm"

if [ -n "$as_collection" ]; then
    features=collection
    for name in "${names[@]}"; do
        features="$features,$name"
    done
    (cd "$root" && cargo rustc -p flm-extensions --no-default-features --features "$features" \
        --target "$target" --profile "$profile" --crate-type cdylib)
    mkdir -p "$(dirname "$collection")"
    cp "$built" "$collection"
    echo "build-wasm.sh: wrote ‘${collection}’"
    exit 0
fi

for name in "${names[@]}"; do
    module=$(sed -n 's/^[[:space:]]*module:[[:space:]]*\([^[:space:]#]*\).*/\1/p' "$here/$name/flmextension.yaml" | head -n 1)
    if [ -z "$module" ]; then
        echo "build-wasm.sh: missing key ‘module’ in the manifest of the extension ‘${name}’" >&2
        exit 1
    fi
    (cd "$root" && cargo rustc -p flm-extensions --no-default-features --features "$name" \
        --target "$target" --profile "$profile" --crate-type cdylib)
    mkdir -p "$out/$name"
    cp "$built" "$out/$name/$module"
    for entry in "$here/$name"/*; do
        case "$(basename "$entry")" in
            src | tests) ;;
            *) cp -R "$entry" "$out/$name/" ;;
        esac
    done
    echo "build-wasm.sh: wrote ‘$out/${name}’"
done
