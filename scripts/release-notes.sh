#!/usr/bin/env bash
# Prints the release notes of a version of FLM: the section of docs/_release-notes.md under
# the heading `flm VERSION` (for example `## flm 0.6.0`), up to the next heading of the same
# or a higher level, without its leading and trailing blank lines.
#
#     scripts/release-notes.sh VERSION
#
# The heading's text is `flm` and the version, possibly followed by more words
# (`## flm 0.6.0 (2026-11-26)`). A line inside a fenced code block is never a heading. The
# script exits with status 1 when the file has no such heading or the section is empty, 2
# on a usage error.
#
# The workflow `Release` (.github/workflows/release.yml) gives this text to the GitHub
# release of the version.

set -euo pipefail

if [ $# != 1 ] || [ "$1" = -h ] || [ "$1" = --help ]; then
    sed -n '2,/^$/s/^# \{0,1\}//p' "$0" >&2
    exit 2
fi
version=$1

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
file=docs/_release-notes.md

# Exits with status 3 when the heading is not found; prints nothing when the section is
# empty.
status=0
notes=$(awk -v version="$version" '
    # The level of a heading line (its number of leading `#`), 0 for any other line.
    { level = (!fenced && $0 ~ /^#+[ \t]/) ? length($1) : 0 }
    /^[ \t]*(```|~~~)/ { fenced = !fenced }
    found && level && level <= found_level { exit }
    found { lines[++n] = $0; next }
    level && $2 == "flm" && $3 == version { found = 1; found_level = level }
    END {
        if (!found) exit 3
        first = 1
        while (first <= n && lines[first] ~ /^[ \t]*$/) first++
        last = n
        while (last >= first && lines[last] ~ /^[ \t]*$/) last--
        for (i = first; i <= last; i++) print lines[i]
    }
' "$root/$file") || status=$?

if [ "$status" = 3 ]; then
    echo "release-notes.sh: Missing heading ‘flm ${version}’ in ‘${file}’" >&2
    exit 1
elif [ "$status" != 0 ]; then
    exit "$status"
elif [ -z "$notes" ]; then
    echo "release-notes.sh: Empty release notes under heading ‘flm ${version}’ in ‘${file}’" >&2
    exit 1
fi
printf '%s\n' "$notes"
