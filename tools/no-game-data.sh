#!/bin/sh
# Fails when the tree holds something that must not be published: big files, game or toolchain
# artefacts by extension, private absolute paths, attribution trailers, key material patterns.
# Usage: tools/no-game-data.sh   (run from anywhere)
set -eu
cd "$(dirname "$0")/.."
fail=0
bad() { printf 'no-game-data: %s\n' "$*" >&2; fail=1; }

files=$(if git rev-parse --git-dir >/dev/null 2>&1; then git ls-files; else find . -type f -not -path './target/*' -not -path './.git/*' | sed 's|^\./||'; fi)

# files over 1 MB
for f in $files; do
  [ -f "$f" ] || continue
  size=$(wc -c < "$f")
  [ "$size" -gt 1048576 ] && bad "$f is larger than 1 MB"
done

# forbidden extensions
for f in $files; do
  case "$f" in
    *.nca|*.nsp|*.xci|*.nsz|*.keys|*.spv|*.air|*.metallib|*.ipa) bad "$f has a forbidden extension" ;;
  esac
done

# forbidden strings (this script is excluded)
scan() {
  pattern=$1; label=$2
  hits=$(echo "$files" | grep -v '^tools/no-game-data.sh$' | while read -r f; do [ -f "$f" ] && grep -InE -- "$pattern" "$f" /dev/null || true; done)
  if [ -n "$hits" ]; then printf '%s\n' "$hits" >&2; bad "found $label"; fi
}
scan '/Users/|/Volumes/' 'a private absolute path'
scan 'Co-Authored-By|Generated with' 'an attribution trailer'
scan '(title|master)_?key[^[:alnum:]]*[=:][[:space:]]*[0-9a-fA-F]{32}' 'key material'
scan '-----BEGIN [A-Z ]*PRIVATE KEY' 'a private key block'

[ "$fail" -eq 0 ] && echo "no-game-data: ok"
exit "$fail"
