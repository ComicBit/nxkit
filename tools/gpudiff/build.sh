#!/bin/sh
# Builds bin/gpudiff (Swift + Metal, no packages)
set -e
cd "$(dirname "$0")"
mkdir -p bin
xcrun swiftc -O -swift-version 5 src/*.swift -o bin/gpudiff -framework Metal
