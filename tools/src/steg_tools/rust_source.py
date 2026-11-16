"""Read every `.rs` file under one folder into plain (path, text) pairs.

Shared by `ui_levels` (R119) and `ui_colours` (R120): both scan `crates/ui`'s Rust
source and need nothing more than a file's path, relative to the repository root, and
its text.
"""

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class RustFile:
    """One `.rs` file: its path relative to the repository root (posix, forward slashes), and its text."""

    path: str
    text: str


def rust_files(root: Path, under: str) -> tuple[RustFile, ...]:
    """Read every `.rs` file under `root/under`, sorted by path.

    `under` that doesn't exist (a crate not built yet) reads as no files, not an error -
    nothing here is reporting that `crates/ui` is missing, only what it contains.
    """
    base = root / under
    if not base.is_dir():
        return ()
    return tuple(
        RustFile(path.relative_to(root).as_posix(), path.read_text()) for path in sorted(base.rglob("*.rs"))
    )
