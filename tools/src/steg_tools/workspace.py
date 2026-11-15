"""Shared access to this repository's root, its workspace's `cargo metadata`, and its justfile's `just --dump`.

Every tool that reads the workspace's crates (steg-affected, steg-boundaries) does it
through here, so there is one place that knows how to find the repository and run
`cargo metadata`, not one copy per tool. steg-recipes reads the justfile the same way,
through `just_dump`.
"""

import json
import subprocess
from pathlib import Path


def git(*args: str) -> list[str]:
    """Run git and return its stdout as non-empty lines, or an empty list if it failed."""
    done = subprocess.run(["git", *args], capture_output=True, text=True)
    return [line for line in done.stdout.splitlines() if line] if done.returncode == 0 else []


def repo_root() -> Path | None:
    """Find the repository root, or None if this isn't a git checkout."""
    top = git("rev-parse", "--show-toplevel")
    return Path(top[0]) if top else None


def cargo_metadata(root: Path) -> dict[str, object] | None:
    """Run `cargo metadata` for the workspace at `root`, or None if it fails."""
    done = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        capture_output=True,
        text=True,
        cwd=root,
    )
    return json.loads(done.stdout) if done.returncode == 0 else None


def just_dump(root: Path) -> dict[str, object] | None:
    """Run `just --dump --dump-format json` for the justfile at `root`, or None if it fails."""
    done = subprocess.run(
        ["just", "--justfile", str(root / "justfile"), "--dump", "--dump-format", "json"],
        capture_output=True,
        text=True,
        cwd=root,
    )
    return json.loads(done.stdout) if done.returncode == 0 else None
