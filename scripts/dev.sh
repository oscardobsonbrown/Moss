#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

# Keep development data separate: incompatible edits can stop the process without a save.
export MOSS_DATA_DIR="${MOSS_DATA_DIR:-$PWD/target/dev-data}"
export MOSS_DEMO="${MOSS_DEMO:-1}"
export CARGO_BUILD_JOBS=2
export RUNE_DX="${RUNE_DX:-${CARGO_HOME:-$HOME/.cargo}/bin/dx}"

if [ ! -x "$RUNE_DX" ]; then
    echo 'Install Dioxus CLI 0.7.10: cargo install dioxus-cli --version 0.7.10 --locked --jobs 2' >&2
    exit 1
fi
case "$("$RUNE_DX" --version)" in
    'dioxus 0.7.10'|'dioxus 0.7.10 ('*) ;;
    *) echo 'Rune requires Dioxus CLI 0.7.10. Set RUNE_DX to that executable.' >&2; exit 1 ;;
esac

# Resolve the launcher from the same pinned Rune source that Cargo builds.
rune_root=$(cargo metadata --locked --format-version 1 | python3 -c '
import json, pathlib, sys
packages = json.load(sys.stdin)["packages"]
print(pathlib.Path(next(p["manifest_path"] for p in packages if p["name"] == "rune")).parents[2])
')
exec python3 "$rune_root/scripts/rune-dev" --local \
    --manifest-path "$PWD/crates/moss-app/Cargo.toml" \
    --patch 'src/ui/**/*.rs' --patch src/rich_text.rs --port 18745 -- "$@"
