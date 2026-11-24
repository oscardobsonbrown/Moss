#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
printf '%s\n' "Configured git hooks path to .githooks"
