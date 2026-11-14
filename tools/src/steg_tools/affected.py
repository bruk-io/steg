"""Say which crates a change affects, so only their tests run.

Prints one of:
  all        the change touches something every crate depends on
  none       the change touches no crate (docs, the board, rules)
  -p a -p b  the crates that changed, and the crates that depend on them

The change is everything on this branch since it left the base (origin/main by
default), plus uncommitted and untracked files. Nobody chooses the tests by hand:
`just test` passes this straight to cargo.

Usage: steg-affected [--base <ref>]
"""

import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

# A change to any of these can change every crate's build or behaviour.
GLOBAL_FILES = frozenset({"Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", "deny.toml", "justfile"})
GLOBAL_FOLDERS = (".cargo/", "tools/")


@dataclass(frozen=True)
class Crate:
    """A workspace crate: its name, its folder from the repository root, and the workspace crates it depends on."""

    name: str
    folder: str
    depends_on: frozenset[str]


def is_global(path: str) -> bool:
    """Say whether a changed file can affect every crate."""
    return path in GLOBAL_FILES or path.startswith(GLOBAL_FOLDERS)


def owner(path: str, crates: tuple[Crate, ...]) -> str | None:
    """Name the crate a file belongs to: the one whose folder is the longest prefix of its path."""
    best: Crate | None = None
    for crate in crates:
        prefix = "" if crate.folder in ("", ".") else crate.folder.rstrip("/") + "/"
        if prefix and path.startswith(prefix) and (best is None or len(crate.folder) > len(best.folder)):
            best = crate
    return best.name if best else None


def with_dependents(changed: frozenset[str], crates: tuple[Crate, ...]) -> frozenset[str]:
    """Add every crate that depends, directly or through others, on a changed crate."""
    affected = set(changed)
    grew = True
    while grew:
        grew = False
        for crate in crates:
            if crate.name not in affected and crate.depends_on & affected:
                affected.add(crate.name)
                grew = True
    return frozenset(affected)


def selection(changed_files: tuple[str, ...], crates: tuple[Crate, ...]) -> str:
    """Decide what to test for a set of changed files."""
    if any(is_global(path) for path in changed_files):
        return "all"
    touched = frozenset(name for path in changed_files if (name := owner(path, crates)) is not None)
    if not touched:
        return "none"
    return " ".join(f"-p {name}" for name in sorted(with_dependents(touched, crates)))


def crates_from_metadata(metadata: dict[str, object], root: Path) -> tuple[Crate, ...]:
    """Read the workspace's crates and the edges between them from `cargo metadata` output."""
    members = set(metadata["workspace_members"])  # type: ignore[arg-type]
    packages = [p for p in metadata["packages"] if p["id"] in members]  # type: ignore[union-attr,index]
    names = {p["name"] for p in packages}
    found = []
    for package in packages:
        folder = Path(package["manifest_path"]).parent.resolve().relative_to(root.resolve()).as_posix()
        depends_on = frozenset(d["name"] for d in package["dependencies"] if d.get("path") and d["name"] in names)
        found.append(Crate(package["name"], folder, depends_on))
    return tuple(found)


def _git(*args: str) -> list[str]:
    done = subprocess.run(["git", *args], capture_output=True, text=True)
    return [line for line in done.stdout.splitlines() if line] if done.returncode == 0 else []


def changed_files(base: str) -> tuple[str, ...]:
    """List files changed on this branch since the base, plus uncommitted and untracked ones."""
    merge_base = _git("merge-base", base, "HEAD")
    since = _git("diff", "--name-only", f"{merge_base[0]}...HEAD") if merge_base else []
    uncommitted = _git("diff", "--name-only", "HEAD")
    untracked = _git("ls-files", "--others", "--exclude-standard")
    return tuple(sorted({PurePosixPath(p).as_posix() for p in (*since, *uncommitted, *untracked)}))


def main() -> int:
    """Print the selection for the current change."""
    base = sys.argv[2] if len(sys.argv) == 3 and sys.argv[1] == "--base" else "origin/main"
    top = _git("rev-parse", "--show-toplevel")
    if not top:
        print("steg-affected: not in a git repository", file=sys.stderr)
        return 2
    root = Path(top[0])
    if not (root / "Cargo.toml").is_file():
        print("none")
        return 0
    done = subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], capture_output=True, text=True, cwd=root)
    if done.returncode != 0:
        print(f"steg-affected: cargo metadata failed, so every test runs:\n{done.stderr.strip()}", file=sys.stderr)
        print("all")
        return 0
    print(selection(changed_files(base), crates_from_metadata(json.loads(done.stdout), root)))
    return 0

