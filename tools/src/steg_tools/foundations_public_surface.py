"""The compile-level claim doc-17 R123 makes and `crates/ui/src/foundations/mod.rs`'s own
doc comment repeats - "no public function returns a bare light-or-dark form outside a
`Theme` method" - isn't something Rust's own type system proves on its own (nothing stops
a *new* free function from being added returning `Hsla`), so this module checks it as text
instead: review #5 of TASK-1.93 found exactly this claim false once (a public
`default_session(Appearance) -> Hsla`, found by calling it from outside `foundations`) and
asked for the check the claim deserves.

What this checks, precisely: every `pub fn ... -> Hsla` in
`crates/ui/src/foundations/mod.rs` and `crates/ui/src/foundations/generated.rs`, found by
regex over the raw source text, that falls outside every brace-matched `impl Theme { ... }`
block in that same text. A violation names the function.

What this cannot see, named outright: a function returning `Hsla` wrapped in another type
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

from steg_tools import workspace

REPO_ROOT_RELATIVE_MOD_FILE = "crates/ui/src/foundations/mod.rs"
REPO_ROOT_RELATIVE_GENERATED_FILE = "crates/ui/src/foundations/generated.rs"

_PUB_FN_RETURNING_HSLA = re.compile(r"pub fn (\w+)\([^)]*\)(?:\s*->\s*Hsla)\b")
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


def main() -> int:
    """Check the real `foundations` module's own two hand-written-plus-generated files."""
    root = workspace.repo_root()
    if root is None:
        print("steg-foundations-public-surface: not in a git repository", file=sys.stderr)
        return 2
    files = (REPO_ROOT_RELATIVE_MOD_FILE, REPO_ROOT_RELATIVE_GENERATED_FILE)
    found = []
    for relative_path in files:
        path = root / relative_path
        if not path.exists():
            continue
        found.extend(bare_hsla_returning_functions(relative_path, path.read_text()))
    for violation in found:
        print(format_violation(violation))
    return 1 if found else 0
