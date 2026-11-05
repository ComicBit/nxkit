#!/bin/sh
# Token-for-token comparison of the Rust lexer with msl_lex.py.
# usage: tools/compare_tokens.sh <file list> <work dir>
# Writes <work dir>/tok_py/<n>.tok and <work dir>/tok_rs/<n>.tok and compares them byte by byte.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
list=$1; work=$2
target=${CARGO_TARGET_DIR:-target}
rm -rf "$work/tok_py" "$work/tok_rs"
python3 "$here/lex_dump.py" "$list" "$work/tok_py"
"$target/release/mslcheck" --lexdump "$work/tok_rs" -l "$list"
if diff -rq "$work/tok_py" "$work/tok_rs"; then
    echo "identical: $(ls "$work/tok_py" | wc -l | tr -d ' ') files, $(cat "$work"/tok_py/*.tok | wc -l | tr -d ' ') tokens"
else
    echo "token dumps differ"; exit 1
fi
