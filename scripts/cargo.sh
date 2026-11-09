#!/bin/sh
# Prefer the user's toolchain; allow the isolated toolchain used for this prototype.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if command -v cargo >/dev/null 2>&1; then
    cargo_bin=$(command -v cargo)
elif [ -x "$project_dir/../.toolchain/cargo/bin/cargo" ]; then
    export CARGO_HOME="$project_dir/../.toolchain/cargo"
    export RUSTUP_HOME="$project_dir/../.toolchain/rustup"
    cargo_bin="$CARGO_HOME/bin/cargo"
else
    echo 'Rust est requis. Installez la chaîne stable depuis https://rustup.rs.' >&2
    exit 1
fi
cd "$project_dir"
if [ "$#" -eq 0 ]; then set -- run --locked; fi
exec "$cargo_bin" "$@"
