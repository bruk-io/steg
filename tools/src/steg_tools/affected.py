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

import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

from steg_tools import workspace

# A change to any of these can change every crate's build or behaviour.
GLOBAL_FILES = frozenset(
    {
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain",
        "rust-toolchain.toml",
        "deny.toml",
        "boundaries.toml",
        "justfile",
        "mise.toml",
    }
)
# third_party/ is the vendored ghostty source and Zig packages (TASK-1.34): no crate's folder
# is a prefix of it, so `owner()` would otherwise attribute a change there to nothing, and a
# bump to the vendored commit or its Zig dependencies would silently select `test: none`.
GLOBAL_FOLDERS = (".cargo/", "tools/", "third_party/")


@dataclass(frozen=True)
class Crate:
    """A workspace crate: its name, its folder, the workspace crates it depends on, and every dependency's name.

    `depends_on` is workspace-internal only (path dependencies) and drives `with_dependents`/`selection`.
    `dependencies` is every direct dependency, workspace and external alike, and is what the boundary
    check in steg_tools.boundaries reads; it plays no part in picking which tests to run.

    `dependency_features` and `default_features_on` are declared, not resolved: each comes straight
    from one dependency edge's own `features`/`uses_default_features` entry in `cargo metadata
    --no-deps`, unaffected by Cargo's workspace-wide feature unification (one crate's edge can say
    `features: []` while the same dependency is built with a feature on elsewhere in the same
    `--workspace` build). `steg_tools.boundaries`'s `deny-features` check reads these two; nothing
    else does, so both default to empty for every other caller and fixture.
    """

    name: str
    folder: str
    depends_on: frozenset[str]
    dependencies: frozenset[str]
    dependency_features: frozenset[tuple[str, str]] = frozenset()
    default_features_on: frozenset[str] = frozenset()


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
        dependencies = frozenset(d["name"] for d in package["dependencies"])
        depends_on = frozenset(d["name"] for d in package["dependencies"] if d.get("path") and d["name"] in names)
        dependency_features = frozenset(
            (d["name"], feature) for d in package["dependencies"] for feature in d.get("features", [])
        )
        default_features_on = frozenset(
            d["name"] for d in package["dependencies"] if d.get("uses_default_features", True)
        )
        found.append(Crate(package["name"], folder, depends_on, dependencies, dependency_features, default_features_on))
    return tuple(found)


def changed_files(base: str) -> tuple[str, ...]:
    """List files changed on this branch since the base, plus uncommitted and untracked ones."""
    merge_base = workspace.git("merge-base", base, "HEAD")
    since = workspace.git("diff", "--name-only", f"{merge_base[0]}...HEAD") if merge_base else []
    uncommitted = workspace.git("diff", "--name-only", "HEAD")
    untracked = workspace.git("ls-files", "--others", "--exclude-standard")
    return tuple(sorted({PurePosixPath(p).as_posix() for p in (*since, *uncommitted, *untracked)}))


def main() -> int:
    """Print the selection for the current change."""
    base = sys.argv[2] if len(sys.argv) == 3 and sys.argv[1] == "--base" else "origin/main"
    root = workspace.repo_root()
    if root is None:
        print("steg-affected: not in a git repository", file=sys.stderr)
        return 2
    if not (root / "Cargo.toml").is_file():
        print("none")
        return 0
    metadata = workspace.cargo_metadata(root)
    if metadata is None:
        print("steg-affected: cargo metadata failed, so every test runs", file=sys.stderr)
        print("all")
        return 0
    print(selection(changed_files(base), crates_from_metadata(metadata, root)))
    return 0

