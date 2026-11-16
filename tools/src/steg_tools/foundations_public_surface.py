"""Enforces doc-17 R123 ("no public function returns a bare light-or-dark colour outside
a `Theme` method") as a check in `just check` (`check-foundations-public-surface`) -
Rust's own type system doesn't prove this on its own (nothing stops a *new* free function
from being added returning `Hsla`), and review #5 of TASK-1.93 found the claim false once
(a public `default_session(Appearance) -> Hsla`, found by calling it from outside
`foundations`) before this check existed.

What this checks, precisely, and no more than this: every `pub fn` whose *declared*
return type is spelled `Hsla` or `gpui::Hsla` - matched as text, by regex, over the raw
source of every `.rs` file under `crates/ui/src/**` and `crates/ui/gallery/**` (the
crate's whole public surface, not just `foundations` - R123 is about the crate; review
#5's actual finding was in `foundations/mod.rs` itself) - that falls outside every
brace-matched `impl Theme { ... }` block in that same file. A violation names the file
and the function.

What this cannot see, named outright - review #6 of TASK-1.93 found all three by getting
past the check with each in turn, not by inspection:
- a `pub const` of the colour type (`pub const X: Hsla = ...;`) - this check only ever
  looks at `fn` signatures, never a `const`'s declared type.
- a renamed import of the type (`use gpui::Hsla as Colour;`, then `pub fn f() -> Colour`)
  - the regex matches the literal text `Hsla`/`gpui::Hsla`, not whatever other name the
    same type is imported under.
- a `type` alias for it (`type ColourAlias = Hsla;`, then `pub fn f() -> ColourAlias`) -
  same reason: the alias's own name is what appears in the signature, not `Hsla`.

Also unseen, from before review #6: a function returning `Hsla` wrapped in another type
(a tuple, `Option<Hsla>`, a struct with an `Hsla` field) - `hex_of`, `tint()` and `scrim()`
all do this legitimately and are invisible to this check by design, since R123 is about
handing out a *bare* colour, not merely touching one. A multi-line function signature whose
parameter list itself contains a `)` (none exist in this crate today) would also not match
the single-line regex here - a real limitation, not a design choice, flagged rather than
silently accepted.

Usage: steg-foundations-public-surface
"""

import re
import sys
from dataclasses import dataclass
from pathlib import Path

from steg_tools import workspace

# Relative to the repo root; both exist today (TASK-1.93) and together are the whole of
# this crate's source - kept as two directories, not individual file names, so a new
# atom/molecule/organism file is covered without this module needing to know its name.
REPO_ROOT_RELATIVE_SRC_DIR = "crates/ui/src"
REPO_ROOT_RELATIVE_GALLERY_DIR = "crates/ui/gallery"

_PUB_FN_RETURNING_HSLA = re.compile(r"pub fn (\w+)\([^)]*\)(?:\s*->\s*(?:gpui::)?Hsla)\b")
_IMPL_THEME_OPEN = re.compile(r"impl\s+Theme\s*\{")


@dataclass(frozen=True)
class Violation:
    """One `pub fn` outside `impl Theme` whose return type is a bare `Hsla`."""

    file: str
    name: str


def _impl_theme_spans(source: str) -> tuple[tuple[int, int], ...]:
    """The (start, end) character ranges of every brace-matched `impl Theme { ... }`
    block in `source`, found by counting braces from each `impl Theme {` forward."""
    spans = []
    for match in _IMPL_THEME_OPEN.finditer(source):
        depth = 1
        position = match.end()
        while depth > 0 and position < len(source):
            if source[position] == "{":
                depth += 1
            elif source[position] == "}":
                depth -= 1
            position += 1
        spans.append((match.start(), position))
    return tuple(spans)


def bare_hsla_returning_functions(file: str, source: str) -> tuple[Violation, ...]:
    """Every `pub fn ... -> Hsla` in `source` that falls outside every `impl Theme`
    block in it, named `file` in the returned `Violation`s."""
    theme_spans = _impl_theme_spans(source)
    found = []
    for match in _PUB_FN_RETURNING_HSLA.finditer(source):
        inside_theme = any(start <= match.start() < end for start, end in theme_spans)
        if not inside_theme:
            found.append(Violation(file, match.group(1)))
    return tuple(found)


def format_violation(violation: Violation) -> str:
    """Report one violation, naming the function and R123."""
    return (
        f"{violation.file}: pub fn {violation.name}(...) -> Hsla is outside impl Theme - "
        "doc-17 R123 refuses a public function that hands out a bare light-or-dark colour; "
        "give it through a Theme method instead."
    )


def rust_files(root: Path, *relative_dirs: str) -> tuple[Path, ...]:
    """Every `.rs` file under each of `relative_dirs`, relative to `root`."""
    return tuple(path for relative in relative_dirs for path in sorted((root / relative).rglob("*.rs")))


def main() -> int:
    """Check every `.rs` file in the real `steg-ui` crate - its `src/` and its gallery."""
    root = workspace.repo_root()
    if root is None:
        print("steg-foundations-public-surface: not in a git repository", file=sys.stderr)
        return 2
    found = []
    for path in rust_files(root, REPO_ROOT_RELATIVE_SRC_DIR, REPO_ROOT_RELATIVE_GALLERY_DIR):
        found.extend(bare_hsla_returning_functions(str(path.relative_to(root)), path.read_text()))
    for violation in found:
        print(format_violation(violation))
    return 1 if found else 0
