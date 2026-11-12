#!/bin/sh
set -eu

source_app=$1
target_app=$2
install_dir=$(dirname "$target_app")
staging_dir=$(mktemp -d "$install_dir/.moss-install.XXXXXX")
previous_app="$staging_dir/previous.app"

cleanup() {
    if [ -e "$previous_app" ] || [ -L "$previous_app" ]; then
        if [ ! -e "$target_app" ] && [ ! -L "$target_app" ]; then
            if ! mv "$previous_app" "$target_app"; then
                printf '%s\n' "Could not restore the previous app. It is at $previous_app" >&2
                return
            fi
        fi
    fi
    rm -rf "$staging_dir"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

ditto "$source_app" "$staging_dir/Moss.app"
codesign --verify --deep --strict "$staging_dir/Moss.app"
if [ -e "$target_app" ] || [ -L "$target_app" ]; then
    mv "$target_app" "$previous_app"
fi
mv "$staging_dir/Moss.app" "$target_app"
printf '%s\n' "Installed $target_app. Restart Moss to use the new build."
