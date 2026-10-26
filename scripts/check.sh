#!/usr/bin/env bash
# Runs the checks of the repository: the tests, clippy, the documentation build, the
# minimum supported Rust versions, the builds with features turned off, the WebAssembly
# builds, and the test suites that need an outside program (LaTeX, Chrome, Node).
#
#     scripts/check.sh [OPTIONS] [PART ...]
#
# The checks are grouped in parts; `scripts/check.sh --list` lists them with what each
# runs. With no PART, the script runs the part group `quick` (the parts `test`, `clippy`,
# `docs` and `msrv`, what the continuous integration runs on every push); `all` names every
# part. The parts run in the order of the list, whatever the order they are given in.
#
# Every command runs, even after one fails. Each command's output is shown and also
# written to its own log file, `LOGDIR/NAME.log`, and one line per command, with its
# outcome and test counts, is added to `LOGDIR/summary.txt`, whose last line is
# `CHECKS PASSED` or `CHECKS FAILED`. LOGDIR is `target/check/` of the checkout
# (`$CARGO_TARGET_DIR/check/` when that variable is set). The script exits with status 0
# when every command passed, 1 otherwise, 2 on a usage error.
#
# Options:
#
#     --locked          pass --locked to cargo: the committed Cargo.lock must be up to date
#     --strict          count a test that was skipped for a missing program as a failure
#     --log-dir DIR     write the logs into DIR instead of LOGDIR
#     -q, --quiet       write each command's output to its log only, not to the terminal
#     -n, --dry-run     print the commands without running them
#     --list            list the parts and the commands of each
#     -h, --help        print this help
#
# What a part needs besides a stable Rust toolchain:
#
# - `msrv`: the toolchains 1.86 and 1.88 (`rustup toolchain install 1.86 1.88`), the
#   floors that the crates declare in their `rust-version` (1.86 for the library crates
#   flm-core, flm-phfpack and flm-info, 1.88 for the others);
# - `wasm`: the target `wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`);
# - `latex`: `pdflatex`, `lualatex` and `latexmk` on the PATH, with the LaTeX packages the
#   generated documents load (a full TeX Live has them); on macOS, the MacTeX folder
#   /Library/TeX/texbin is added to the PATH when `pdflatex` is not found on it;
# - `browser`: Google Chrome or Chromium (the variable FLM_TESTS_CHROME may give its path),
#   and MathJax's files, installed by `npm ci` in `npm/`;
# - `npm`: Node 22 or later, and the npm workspace's dependencies (`npm ci` in `npm/`).
#
# A test whose program is missing passes after writing a line beginning `skipped:`; the
# summary lists these lines. With --strict, such a command counts as failed: the
# continuous integration, which installs every program it needs, runs the script that way.
#
# In GitHub Actions, each command's output is folded into a group, a failed command is
# reported as an error annotation, and the summary is added to the job summary.

set -u

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root" || exit 2

# The parts, in the order they run.
parts_all="test clippy docs msrv features wasm latex browser npm"
parts_quick="test clippy docs msrv"

# The floors of the minimum supported Rust versions (each crate's `rust-version`).
msrv_library=1.86
msrv_host=1.88

wasm_target=wasm32-unknown-unknown
# `cargo docs` is the workspace alias of .cargo/config.toml. A RUSTDOCFLAGS value replaces
# the rustdoc flags of that file, so the header flag is repeated next to `-D warnings`.
docflags="-D warnings --html-in-header docs/rustdoc-header.html"

usage() {
    sed -n '2,/^$/s/^# \{0,1\}//p' "$0"
}

describe() { # describe <part>
    case "$1" in
        test)     echo "the tests of the workspace, and of flm-info without its optional feature" ;;
        clippy)   echo "clippy with warnings denied" ;;
        docs)     echo "the API documentation with warnings denied" ;;
        msrv)     echo "the crates build with the oldest Rust versions they declare" ;;
        features) echo "the builds and tests with default features turned off, and the parallel build on one thread" ;;
        wasm)     echo "the builds for WebAssembly, and the tests of extensions compiled to WebAssembly" ;;
        latex)    echo "the tests that compile the generated LaTeX documents" ;;
        browser)  echo "the tests of the live preview's page in Chrome without a window" ;;
        npm)      echo "the tests that run the tools of the npm workspace (npm/)" ;;
    esac
}

locked=""
strict=0
quiet=0
dry=0
list=0
logdir=""
requested=""
while [ $# -gt 0 ]; do
    case "$1" in
        --locked) locked="--locked" ;;
        --strict) strict=1 ;;
        -q | --quiet) quiet=1 ;;
        -n | --dry-run) dry=1 ;;
        --list) list=1 ;;
        --log-dir)
            if [ $# -lt 2 ]; then
                echo "check.sh: the option --log-dir must be followed by a folder" >&2
                exit 2
            fi
            logdir=$2
            shift
            ;;
        --log-dir=*) logdir=${1#--log-dir=} ;;
        -h | --help) usage; exit 0 ;;
        -*)
            echo "check.sh: unknown option $1 (see scripts/check.sh --help)" >&2
            exit 2
            ;;
        quick) requested="$requested $parts_quick" ;;
        all) requested="$requested $parts_all" ;;
        *)
            case " $parts_all " in
                *" $1 "*) requested="$requested $1" ;;
                *)
                    echo "check.sh: unknown part $1 (the parts: $parts_all; the groups: quick, all)" >&2
                    exit 2
                    ;;
            esac
            ;;
    esac
    shift
done
[ -n "$requested" ] || requested=$parts_quick

selected() { # selected <part>
    case " $requested " in *" $1 "*) return 0 ;; esac
    return 1
}

if [ "$list" = 1 ]; then
    dry=1
    quiet=1
    requested=$parts_all
fi

if [ -z "$logdir" ]; then
    logdir="${CARGO_TARGET_DIR:-$root/target}/check"
fi
summary="$logdir/summary.txt"
in_actions=0
[ "${GITHUB_ACTIONS:-}" = true ] && in_actions=1

if [ "$dry" = 0 ]; then
    mkdir -p "$logdir" || exit 2
    : > "$summary"
    echo "checks: $(echo $requested), at $(git rev-parse --short HEAD 2>/dev/null) on $(git rev-parse --abbrev-ref HEAD 2>/dev/null), $(date '+%Y-%m-%d %H:%M')" >> "$summary"
fi

failed=0
trap 'echo; echo "check.sh: interrupted"; exit 130' INT TERM

note() { # note <line>: a line of the summary, shown as it is written
    echo "$1" >> "$summary"
    echo "$1"
}

run() { # run <name> <command ...>
    local name=$1
    shift
    if [ "$dry" = 1 ]; then
        printf '  %-24s %s\n' "$name" "$*"
        return 0
    fi
    local log="$logdir/$name.log"
    local status started elapsed
    started=$(date +%s)
    [ "$in_actions" = 1 ] && echo "::group::$name: $*"
    [ "$quiet" = 0 ] && echo "== $name: $*"
    if [ "$quiet" = 1 ]; then
        "$@" > "$log" 2>&1
        status=$?
    else
        "$@" 2>&1 | tee "$log"
        status=${PIPESTATUS[0]}
    fi
    [ "$in_actions" = 1 ] && echo "::endgroup::"
    elapsed=$(( $(date +%s) - started ))

    local counts skips outcome
    counts=$(grep -E '^test result:' "$log" | awk '{ p += $4; f += $6; i += $8 } END { if (NR) printf ", passed %d, failed %d, ignored %d", p, f, i }')
    skips=$(grep -o 'skipped: .*' "$log" | LC_ALL=C sort -u)
    if [ "$status" != 0 ]; then
        outcome=FAILED
    elif [ -n "$skips" ] && [ "$strict" = 1 ]; then
        outcome="FAILED (skipped tests)"
    elif [ -n "$skips" ]; then
        outcome="ok (skipped tests)"
    else
        outcome=ok
    fi
    case "$outcome" in FAILED*) failed=1 ;; esac
    note "$(printf '%-24s %s%s (%ss)' "$name" "$outcome" "$counts" "$elapsed")"
    if [ -n "$skips" ]; then
        printf '%s\n' "$skips" | sed 's/^/    /' >> "$summary"
        [ "$quiet" = 1 ] || printf '%s\n' "$skips" | sed 's/^/    /'
    fi
    case "$outcome" in
        FAILED*) [ "$in_actions" = 1 ] && echo "::error title=check.sh::$name $outcome (log: $log)" ;;
    esac
    return 0
}

part() { # part <name>: the heading of a part
    if [ "$dry" = 1 ]; then
        echo "$1: $(describe "$1")"
    else
        note "-- $1: $(describe "$1")"
    fi
}

if selected test; then
    part test
    # Cargo builds each crate of the workspace once, with every feature that any crate of
    # the workspace turns on: flm-ext with all its features, flm-info with `flm-main`.
    run test-workspace         cargo test $locked --workspace --no-fail-fast
    # flm-info without its feature `flm-main`, the build that needs flm-core alone.
    run test-info-core         cargo test $locked -p flm-info --no-fail-fast
fi

if selected clippy; then
    part clippy
    run clippy-workspace       cargo clippy $locked --workspace --all-targets -- -D warnings
fi

if selected docs; then
    part docs
    run docs-workspace         env RUSTDOCFLAGS="$docflags" cargo docs $locked
fi

if selected msrv; then
    part msrv
    run msrv-library           cargo +$msrv_library check $locked -p flm-core -p flm-phfpack -p flm-info --all-targets
    run msrv-workspace         cargo +$msrv_host check $locked --workspace --all-targets
fi

if selected features; then
    part features
    # flm-core without its default features `parallel` and `docs` (the sequential path).
    # Package-scoped on purpose: in a workspace build, flm-main turns the features back on.
    run test-core-nodefault    cargo test $locked -p flm-core --no-default-features --no-fail-fast
    run clippy-core-nodefault  cargo clippy $locked -p flm-core --all-targets --no-default-features -- -D warnings
    # The parallel build on a pool of one thread.
    run test-core-1thread      env RAYON_NUM_THREADS=1 cargo test $locked -p flm-core --no-fail-fast
    # The host library without the disk, and with each optional feature alone.
    run test-host-nodefault    cargo test $locked -p flm-main --no-default-features --test watch_update --no-fail-fast
    run check-host-graphics    cargo check $locked -p flm-main --no-default-features --features graphics
    run check-host-citations   cargo check $locked -p flm-main --no-default-features --features citations
    run check-host-watch       cargo check $locked -p flm-main --no-default-features --features watch
    # The program without the package `phfpack` and the engines it carries by default.
    run check-cli-nodefault    cargo check $locked -p flm-cli --no-default-features
fi

if selected wasm; then
    part wasm
    run wasm-core              cargo check $locked -p flm-core --no-default-features --target $wasm_target
    run wasm-host              cargo check $locked -p flm-main --no-default-features --target $wasm_target
    run wasm-info              cargo check $locked -p flm-info --no-default-features --target $wasm_target
    # The extension layer without its bindings, and the guest client that an extension
    # compiled to WebAssembly is written against.
    run wasm-ext               cargo check $locked -p flm-ext --no-default-features --target $wasm_target
    run wasm-guest-client      cargo check $locked -p flm-ext --no-default-features --features guest --target $wasm_target
    run wasm-phfpack           cargo check $locked -p flm-phfpack --target $wasm_target
    # The guest crates of flm-ext/tests/wasm/ built for WebAssembly and run through the
    # engine `wasm`; the engine `native` in a program built for WebAssembly.
    run wasm-guest-ext         env FLM_TESTS_WASM_GUEST=1 cargo test $locked -p flm-ext --all-features --test wasm_case1 --test native --test native_wasm32 --no-fail-fast -- --nocapture
    run wasm-guest-cli         env FLM_TESTS_WASM_GUEST=1 cargo test $locked -p flm-cli --test extensions_wasm --no-fail-fast -- --nocapture
    # The extension `profile-pdf-latex` built by extensions/rust/build-wasm.sh gives the PDF
    # of its form compiled into the program. Selected by name: the other gated test of the
    # file needs LaTeX (the part `latex`).
    run wasm-guest-pdf-latex   env FLM_TESTS_WASM_GUEST=1 cargo test $locked -p flm-cli --test pdf_latex --no-fail-fast -- --nocapture the_webassembly_form
fi

if selected latex; then
    part latex
    if [ "$dry" = 0 ] && ! command -v pdflatex > /dev/null 2>&1 && [ -d /Library/TeX/texbin ]; then
        PATH="/Library/TeX/texbin:$PATH"
        export PATH
    fi
    # A test of these suites fails, rather than being skipped, when a LaTeX program is missing.
    run latex-core             env FLM_TESTS_LATEX_COMPILE=1 cargo test $locked -p flm-core --no-fail-fast
    run latex-cli              env FLM_TESTS_LATEX_COMPILE=1 cargo test $locked -p flm-cli --test templates --test end_to_end --test pdf_latex --no-fail-fast
    run latex-phfpack          env FLM_TESTS_LATEX_COMPILE=1 cargo test $locked -p flm-phfpack --test macros --test quantities --no-fail-fast
fi

if selected browser; then
    part browser
    # The feature `js`: the cases of the `math-mathjax` extension's pages are behind it.
    run browser-watch-page     env FLM_TESTS_BROWSER=1 cargo test $locked -p flm-main --features watch,js --test watch_page --no-fail-fast -- --nocapture
fi

if selected npm; then
    part npm
    # The TypeScript check of flm-ext/flm.d.ts.
    run npm-typescript-check   env FLM_TESTS_TYPESCRIPT=1 cargo test $locked -p flm-ext --all-features --test typescript_check --no-fail-fast -- --nocapture
    # The committed lecture-notes bundle is the one its sources build (this test also runs
    # in the part `test`, where it is skipped when the npm workspace is not installed).
    run npm-lecturenotes       cargo test $locked -p flm-cli --test lecturenotes --no-fail-fast -- --nocapture
fi

if [ "$dry" = 1 ]; then
    [ "$list" = 1 ] && echo "groups: quick = $parts_quick; all = every part"
    exit 0
fi

if [ "$failed" = 0 ]; then note "CHECKS PASSED"; else note "CHECKS FAILED"; fi
echo "(summary: $summary)"
if [ "$in_actions" = 1 ] && [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    { echo '```text'; cat "$summary"; echo '```'; } >> "$GITHUB_STEP_SUMMARY"
fi
exit "$failed"
