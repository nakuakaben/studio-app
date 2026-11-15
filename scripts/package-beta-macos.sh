#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary=${1:?Chemin du binaire Studio App requis}
output_dir=${2:?Dossier de sortie requis}
mkdir -p "$output_dir"
output_dir=$(CDPATH= cd -- "$output_dir" && pwd)
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT

app="$staging/Studio App.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/catalog"
install -m 755 "$binary" "$app/Contents/MacOS/studio-app"
for file in config.json catalog.json hyper-media-puller.json; do
    install -m 644 "$project_dir/catalog/$file" "$app/Contents/catalog/$file"
done

python3 - "$project_dir/Cargo.toml" "$app/Contents/Info.plist" <<'PY'
import plistlib
import sys
import tomllib

with open(sys.argv[1], "rb") as manifest:
    version = tomllib.load(manifest)["package"]["version"]
with open(sys.argv[2], "wb") as info:
    plistlib.dump({
        "CFBundleDisplayName": "Studio App",
        "CFBundleExecutable": "studio-app",
        "CFBundleIdentifier": "app.studioapp.launcher",
        "CFBundleName": "Studio App",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "NSHighResolutionCapable": True,
    }, info)
PY

codesign --force --sign - "$app"
codesign --verify --strict "$app"
ln -s /Applications "$staging/Applications"
hdiutil create -ov -volname "Studio App" -srcfolder "$staging" -format UDZO \
    "$output_dir/Studio-App-macos-arm64.dmg"

(
    cd "$output_dir"
    hdiutil verify Studio-App-macos-arm64.dmg
    shasum -a 256 Studio-App-macos-arm64.dmg > SHA256SUMS.txt
)
