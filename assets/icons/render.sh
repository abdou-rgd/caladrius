#!/bin/sh
# Renders the PNG icons of this folder from the logo SVGs in ../logo (the source of truth).
# Needs resvg 0.48.1:  cargo install resvg --version 0.48.1 --locked
# Run from anywhere:   sh assets/icons/render.sh
# The square green background of the logo gets rounded corners (rx = 22 % of the side), as
# desktop and browser icons usually have; nothing else of the drawing changes.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
logo="$here/../logo"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

square='<rect x="0.5" y="89.5" width="1040" height="1040" fill="#83d383"/>'
rounded='<rect x="0.5" y="89.5" width="1040" height="1040" rx="229" fill="#83d383"/>'

# round SOURCE.svg OUT.svg: the logo with rounded corners; fails if the background rect moved.
round() {
    grep -qF "$square" "$1" || { echo "render.sh: background rect not found in $1" >&2; exit 1; }
    sed "s|$square|$rounded|" "$1" > "$2"
}

round "$logo/caladrius-logo.svg" "$tmp/full.svg"
round "$logo/caladrius-logo-small.svg" "$tmp/small.svg"

# The full drawing for large sizes, the simplified one below about 64 px (see ../logo/README.md).
resvg -w 256 -h 256 "$tmp/full.svg" "$here/caladrius-icon-256.png"
resvg -w 64 -h 64 "$tmp/small.svg" "$here/caladrius-icon-64.png"
resvg -w 32 -h 32 "$tmp/small.svg" "$here/caladrius-icon-32.png"
echo "render.sh: wrote caladrius-icon-256.png, caladrius-icon-64.png, caladrius-icon-32.png"
