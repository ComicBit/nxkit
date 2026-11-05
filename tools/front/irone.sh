#!/bin/sh
# irone.sh SRC.metal [irdiff args]: own IR of one file through air-opt -O1, irdiff against A3's stored reference.
f=$1; shift
d=$(mktemp -d)
SDK=$(xcrun -sdk macosx --show-sdk-path)
"${NXB_AIRC:-target/release/airc}" emit "$f" -o "$d/raw.ll" || exit 2
env -i SDKROOT="$SDK" "$(xcrun -sdk macosx -find air-opt)" -O1 -S "$d/raw.ll" -o "$d/o.ll" || exit 2
k=$(shasum -a 256 "$f" | cut -c1-64)
python3 "${NXB_REF:?set NXB_REF}/irdiff.py" "$k" "$d/o.ll" "$@"
