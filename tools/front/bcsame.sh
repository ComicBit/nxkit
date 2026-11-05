#!/bin/sh
# bcsame.sh OLD_AIRC NEW_AIRC LIST [ios]: bitcode of every source from two airc builds, byte compared (front-end change
# with unchanged output = unchanged library bytes). Prints "same N differ M fail K" and the differing paths.
OLD=$1; NEW=$2; LIST=$3; X=${4:+--ios}
T=$(mktemp -d); s=0; d=0; f=0
while read -r p; do
  "$OLD" bc $X "$p" -o "$T/a.bc" 2>/dev/null; ra=$?
  "$NEW" bc $X "$p" -o "$T/b.bc" 2>/dev/null; rb=$?
  if [ $ra -ne 0 ] || [ $rb -ne 0 ]; then [ $ra -eq $rb ] && f=$((f+1)) || { d=$((d+1)); echo "DIFF-STATUS $p"; }; continue; fi
  if cmp -s "$T/a.bc" "$T/b.bc"; then s=$((s+1)); else d=$((d+1)); echo "DIFF $p"; fi
done < "$LIST"
rm -rf "$T"; echo "same $s differ $d fail $f"
