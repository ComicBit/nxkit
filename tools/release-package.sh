#!/bin/sh
# Packs the release: signs the binaries, makes the ZIP file, the checksum file and the release notes.
# Usage: tools/release-package.sh TAG            (for example v0.1.0, run after cargo build --release)
# Environment: NXB_RELEASE_BINS (names of the binaries, default "nxb nxb-extract nxb-split nxb-ios airc"), TARGET_DIR (default target).
# Output in dist/: nxb-TAG-macos-arm64.zip, SHA256SUMS, notes.md
set -eu
cd "$(dirname "$0")/.."
tag=${1:?usage: release-package.sh TAG}
bins=${NXB_RELEASE_BINS:-"nxb nxb-extract nxb-split nxb-ios airc"}
target=${TARGET_DIR:-${CARGO_TARGET_DIR:-target}}/release
version=${tag#v}
name="nxb-${tag}-macos-arm64"

rm -rf dist; mkdir -p "dist/$name"
for b in $bins; do
  test -x "$target/$b" || { echo "missing binary: $target/$b" >&2; exit 1; }
  cp "$target/$b" "dist/$name/$b"
  codesign -s - --force "dist/$name/$b"
  codesign --verify "dist/$name/$b"
done
cp LICENSE THIRD_PARTY.md "dist/$name/"
( cd dist && zip -qr "$name.zip" "$name" && shasum -a 256 "$name.zip" > SHA256SUMS \
  && ( cd "$name" && shasum -a 256 $bins ) | sed "s|  |  $name/|" >> SHA256SUMS && cat SHA256SUMS )

# notes: the section of CHANGELOG.md for this version
awk -v v="$version" '
  $0 ~ "^## " v "( |$)" { on = 1; next }
  on && /^## / { exit }
  on { print }
' CHANGELOG.md > dist/notes.md
test -s dist/notes.md || { echo "no section for $version in CHANGELOG.md" >&2; exit 1; }
echo "release files are in dist/"
