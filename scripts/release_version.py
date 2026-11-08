#!/usr/bin/env python3
"""Choose the next minor release, or reuse the release for this commit."""

import re
import subprocess
import sys
import tomllib
from pathlib import Path

VERSION = re.compile(r"v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)\Z")


def parsed(tags):
    return [tuple(map(int, match.groups())) for tag in tags if (match := VERSION.fullmatch(tag))]


def choose(tags, head_tags, baseline):
    existing = parsed(head_tags)
    if existing:
        return "v" + ".".join(map(str, max(existing)))
    major, minor, _ = max([baseline, *parsed(tags)])
    return f"v{major}.{minor + 1}.0"


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: release_version.py COMMIT")
    root = Path(__file__).resolve().parent.parent
    with (root / "Cargo.toml").open("rb") as file:
        version = tomllib.load(file)["workspace"]["package"]["version"]
    baseline = parsed([f"v{version}"])
    if len(baseline) != 1:
        raise SystemExit("Workspace version must be a stable semantic version")

    def git_tags(*args):
        return subprocess.check_output(["git", "tag", "--list", *args], cwd=root, text=True).splitlines()

    print(choose(git_tags(), git_tags("--points-at", sys.argv[1]), baseline[0]))


if __name__ == "__main__":
    main()
