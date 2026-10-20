#!/usr/bin/env bash
# Builds the WebAssembly form of the extensions written in Rust in this folder.
#
#     extensions/rust/build-wasm.sh [--out DIR] [NAME ...]
#
# For each extension NAME (every folder of this folder holding a `Cargo.toml` when no name
# is given), the script runs
#
#     cargo rustc -p NAME --target wasm32-unknown-unknown --profile wasm-extension --crate-type cdylib
#
# from the root of the checkout, and writes the folder DIR/NAME/ holding the extension's
# `flmextension.yaml`, its `README.md` and its module (NAME with each `-` replaced by `_`,
# then `.wasm`: `profile_pdf_latex.wasm`, the module that the manifest specifies). DIR defaults to
# `target/wasm-extensions/` of the checkout. The folder DIR/NAME is an extension as the
# `flm` program finds it on its search path: `flm render --extensions-dir DIR …`.
#
# Each extension is built in a cargo run of its own, with its package selected alone:
# within one cargo run, `flm-ext` is compiled once, with one set of features, and a run
# that also builds the `flm` program would turn on `flm-ext`'s feature `native`, which
# leaves the module without its exports. The profile `wasm-extension` is declared in the
# workspace's `Cargo.toml` (the release profile, optimized for size, aborting on a panic).
# The cargo target folder is the one cargo uses for the checkout (`CARGO_TARGET_DIR` when
# it is set).
#
# Neither a cargo build nor the tests run this script, except the test of the WebAssembly
# form in `flm-cli/tests/pdf_latex.rs`, gated on `FLM_TESTS_WASM_GUEST=1`.

set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
target=wasm32-unknown-unknown
profile=wasm-extension

usage() {
    echo "usage: extensions/rust/build-wasm.sh [--out DIR] [NAME ...]"
    echo "Builds the WebAssembly form of the extensions written in Rust under extensions/rust/"
    echo "(all of them when no NAME is given) into DIR/NAME/ (DIR: target/wasm-extensions/ of the checkout)."
}

out="$root/target/wasm-extensions"
names=()
while [ $# -gt 0 ]; do
    case "$1" in
        --out)
            if [ $# -lt 2 ]; then
                echo "build-wasm.sh: the option --out must be followed by a folder" >&2
                exit 2
            fi
            out=$2
            shift 2
            ;;
        --out=*)
            out=${1#--out=}
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            echo "build-wasm.sh: the option $1 is not one of --out and --help" >&2
            exit 2
            ;;
        *)
            names+=("$1")
            shift
            ;;
    esac
done

if [ ${#names[@]} -eq 0 ]; then
    for folder in "$here"/*/; do
        if [ -f "$folder/Cargo.toml" ]; then
            names+=("$(basename "$folder")")
        fi
    done
fi
for name in "${names[@]}"; do
    if [ ! -f "$here/$name/Cargo.toml" ]; then
        echo "build-wasm.sh: the extension $name must be a folder of $here holding a Cargo.toml" >&2
        exit 2
    fi
done

sysroot=$(cd "$root" && rustc --print sysroot)
if [ ! -d "$sysroot/lib/rustlib/$target" ]; then
    echo "build-wasm.sh: the Rust target $target must be installed to build the WebAssembly form of an extension;" \
        "install it with: rustup target add $target" >&2
    exit 1
fi

target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
if [ -z "$target_dir" ]; then
    echo "build-wasm.sh: cargo metadata must give the target folder of the checkout $root" >&2
    exit 1
fi

for name in "${names[@]}"; do
    (cd "$root" && cargo rustc -p "$name" --target "$target" --profile "$profile" --crate-type cdylib)
    module="$(echo "$name" | tr '-' '_').wasm"
    mkdir -p "$out/$name"
    cp "$target_dir/$target/$profile/$module" "$out/$name/$module"
    cp "$here/$name/flmextension.yaml" "$here/$name/README.md" "$out/$name/"
    echo "build-wasm.sh: wrote $out/$name"
done
