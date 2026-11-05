#!/bin/sh
# IR equality of the own front end with Apple's (A3's stored references), after air-opt -O1 on both sides.
# usage: irequal.sh LIST OUT_DIR   (LIST: MSL paths). Candidates are named by the source's sha256 key.
set -e
LIST=$1; OUT=$2; mkdir -p "$OUT/c"
SDK=$(xcrun -sdk macosx --show-sdk-path); OPT=$(xcrun -sdk macosx -find air-opt)
AIRC=${AIRC:-target/release/airc}
while read -r f; do
  k=$(shasum -a 256 "$f" | cut -c1-64)
  "$AIRC" emit "$f" -o "$OUT/raw.ll" 2>/dev/null || continue
  env -i SDKROOT="$SDK" "$OPT" -O1 -S "$OUT/raw.ll" -o "$OUT/c/$k.ll" || rm -f "$OUT/c/$k.ll"
done < "$LIST"
python3 "${NXB_REF:?set NXB_REF}/irdiff.py" --summary --cands "$OUT/c" -j 1 --top 12
