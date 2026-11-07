#!/bin/sh
# Shows how this Mac's GPU sums a half simdgroup_matrix product: run.sh
# Writes only to tools/hmma-precision/bin.
set -e
cd "$(dirname "$0")"
mkdir -p bin
xcrun swiftc -O -swift-version 5 probe.swift -o bin/probe -framework Metal
bin/probe probe.metal
