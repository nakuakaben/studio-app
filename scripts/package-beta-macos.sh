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
mkdir -p "$app/Contents/MacOS" "$app/Contents/catalog" "$app/Contents/Resources"
python3 "$project_dir/scripts/generate-brand-icons.py" "$staging/StudioApp.iconset"
iconutil -c icns -o "$app/Contents/Resources/studio-app.icns" "$staging/StudioApp.iconset"
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
        "CFBundleIconFile": "studio-app.icns",
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
    # The macOS runner can briefly keep the freshly created image busy.
    # Retry a bounded number of times; never accept an unverified image.
    attempt=1
    while ! hdiutil verify Studio-App-macos-arm64.dmg; do
        if [ "$attempt" -ge 3 ]; then
            echo "Échec définitif de la vérification DMG après 3 tentatives" >&2
            exit 1
        fi
        attempt=$((attempt + 1))
        sleep 3
    done
    shasum -a 256 Studio-App-macos-arm64.dmg > SHA256SUMS.txt
)
