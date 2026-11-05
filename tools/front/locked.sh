#!/bin/sh
# locked.sh CMD...: run CMD holding "${NXB_LOCK:-.nxb-bench-lock}" (retry every 2 minutes, released on exit)
L="${NXB_LOCK:-.nxb-bench-lock}"
until mkdir "$L" 2>/dev/null; do sleep 120; done
trap 'rmdir "$L"' EXIT INT TERM
"$@"
