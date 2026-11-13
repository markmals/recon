#!/usr/bin/env bash
# Builds DemoSeeder.app from demo/DemoSeeder with the Swift compiler on this Mac. Run it
# in the guest; it needs no project or package, only the Xcode command-line tools.
#
# usage: build-seeder.sh OUTPUT_DIR      (writes OUTPUT_DIR/DemoSeeder.app)
set -euo pipefail

source=$(cd "$(dirname "$0")/DemoSeeder" && pwd)
output=${1:?usage: build-seeder.sh OUTPUT_DIR}
app="$output/DemoSeeder.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$source/Info.plist" "$app/Contents/Info.plist"
xcrun swiftc -O -swift-version 5 -o "$app/Contents/MacOS/DemoSeeder" "$source"/*.swift
codesign --force --sign - "$app"
echo "$app"
