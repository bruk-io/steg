"""Refuse a colour literal inside `crates/ui` outside foundations (R120): a hex integer,
a hex colour string, a call to one of GPUI's named colour constructors, or a bare
`Hsla`/`Rgba` struct literal.

The constructor and type names are read from the pinned `gpui = 0.2.2`'s own
`src/color.rs` (re-exported at crate root through `pub use color::*;` in `src/gpui.rs`),
not guessed: every free function there that produces a brand-new, fixed colour value
from scratch. `blend`, `opacity`, `alpha`, `fade_out`, `grayscale`, `to_rgb`,
`solid_background`, `linear_gradient`, `linear_color_stop` and `pattern_slash` all take
an *existing* colour as an argument and transform or wrap it - they are not in
`COLOUR_CONSTRUCTORS`, because calling one of them is not itself writing a new literal.

What this cannot see, named outright: a hex value assembled by arithmetic
(`(0x1c << 16) | 0x1b21`) rather than written as one literal; a colour passed as a bare
float triple with no distinguishing call or struct name. A named constant defined
outside foundations and merely *used* elsewhere is not a blind spot - that constant's
own definition is a literal outside foundations too, and is caught there, at its one
true source.

Usage: steg-ui-colours
"""

import re
import sys
from dataclasses import dataclass

from ast_grep_py import SgNode, SgRoot

from steg_tools import foundations, workspace
from steg_tools.rust_source import RustFile, rust_files

UI_SRC = "crates/ui/src"
DESIGN_DATA = "crates/ui/design/foundations.json"

# Permanent: TASK-1.93 writes `crates/ui/src/foundations/generated.rs`, which is exactly
# where every one of these literal forms is meant to live (R121) - the data a *component*
# reads them from, not writes them as.
FOUNDATIONS = f"{UI_SRC}/foundations"

# Temporary - remove this constant, and the line that adds it to EXEMPT_PREFIXES below,
# once TASK-1.93 makes the gallery draw its colours from foundations instead of its own
# placeholder (`crates/ui/CLAUDE.md`, and `gallery/main.rs`'s own doc comment:
# "TASK-1.93's real foundations replace[s]" this placeholder). Whichever of the two
# tasks merges second ends this exemption.
GALLERY = "crates/ui/gallery"

EXEMPT_PREFIXES = (
    FOUNDATIONS,
    GALLERY,  # temporary - remove this line when GALLERY above is removed
)

# Every free function in gpui 0.2.2's `src/color.rs` that builds a brand-new colour
# value from scratch, re-exported at crate root (`pub use color::*;`), read from that
# file directly rather than guessed - see this module's own docstring.
COLOUR_CONSTRUCTORS = frozenset(
    {
        "rgb",
        "rgba",
        "hsla",
        "black",
        "white",
        "red",
        "green",
        "blue",
        "yellow",
        "transparent_black",
        "transparent_white",
        "opaque_grey",
    }
)
# gpui's own colour structs (`src/color.rs`) - a literal struct expression naming either
# is a colour literal regardless of its field values: the one case the hex and call
# checks both miss, a colour built entirely from plain floats with no hex and no named
# call (`Hsla { h: 0.5, s: 0.5, l: 0.5, a: 1.0 }`).
COLOUR_STRUCT_TYPES = frozenset({"Hsla", "Rgba"})

HEX_INT_RE = re.compile(r"^0[xX]([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$")
HEX_STRING_RE = re.compile(r'^"#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})"$')


@dataclass(frozen=True)
class Literal:
    """One colour literal found in a file: its line, its text, and its normalised hex
    value - a named-constructor call with no hex argument, or a bare struct literal,
    has none."""

    line: int
    text: str
    hex_value: str | None


@dataclass(frozen=True)
class Violation:
    """One colour literal outside foundations: where it is, and every foundations value
    (name, "dark"|"light") whose hex it matches - empty when none match."""

    path: str
    line: int
    text: str
    matches: tuple[tuple[str, str], ...]


def is_exempt(path: str) -> bool:
    """Say whether a literal in this file is allowed: see `EXEMPT_PREFIXES` above."""
    return any(path == prefix or path.startswith(f"{prefix}/") for prefix in EXEMPT_PREFIXES)


def _line(node: SgNode) -> int:
    """A node's 1-indexed starting line."""
    return node.range().start.line + 1


def _is_colour_constructor_call(node: SgNode) -> bool:
    """Say whether a `call_expression` node calls one of `COLOUR_CONSTRUCTORS`, however qualified."""
    callee = node.field("function")
    return callee is not None and callee.text().rsplit("::", 1)[-1] in COLOUR_CONSTRUCTORS


def _inside_colour_constructor_call(node: SgNode) -> bool:
    """Say whether `node` is an argument somewhere inside a call already caught by
    `_is_colour_constructor_call` - so a bare `0x1c1b21` inside `rgb(0x1c1b21)` isn't
    reported twice, once for the call and once for its own hex literal."""
    return any(
        ancestor.kind() == "call_expression" and _is_colour_constructor_call(ancestor)
        for ancestor in node.ancestors()
    )


def literal_colours(source: str) -> tuple[Literal, ...]:
    """Every colour literal in one file's source, sorted by line."""
    root = SgRoot(source, "rust").root()
    found: list[Literal] = []
    for node in root.find_all(kind="call_expression"):
        if _is_colour_constructor_call(node):
            found.append(Literal(_line(node), node.text(), None))
    for node in root.find_all(kind="integer_literal"):
        if HEX_INT_RE.match(node.text()) and not _inside_colour_constructor_call(node):
            found.append(Literal(_line(node), node.text(), foundations.normalise_hex(node.text())))
    for node in root.find_all(kind="string_literal"):
        if HEX_STRING_RE.match(node.text()):
            found.append(Literal(_line(node), node.text(), foundations.normalise_hex(node.text().strip('"'))))
    for node in root.find_all(kind="struct_expression"):
        type_node = node.field("name")
        if type_node is not None and type_node.text().rsplit("::", 1)[-1] in COLOUR_STRUCT_TYPES:
            found.append(Literal(_line(node), node.text(), None))
    return tuple(sorted(found, key=lambda literal: literal.line))


def violations(
    files: tuple[RustFile, ...], index: dict[str, tuple[tuple[str, str], ...]]
) -> tuple[Violation, ...]:
    """Say which files have a colour literal outside the exempt paths (R120)."""
    found = []
    for file in files:
        if is_exempt(file.path):
            continue
        for literal in literal_colours(file.text):
            matches = index.get(literal.hex_value, ()) if literal.hex_value is not None else ()
            found.append(Violation(file.path, literal.line, literal.text, matches))
    return tuple(sorted(found, key=lambda violation: (violation.path, violation.line)))


def format_violation(violation: Violation) -> str:
    """Report one violation, naming the matching foundations value where there is one, and R120's own rule."""
    base = f"{violation.path}:{violation.line}: `{violation.text}` is a colour literal, which doc-17 R120 refuses outside foundations."
    if violation.matches:
        names = ", ".join(f"{name} ({form})" for name, form in sorted(violation.matches))
        return f"{base} This value matches {names} - use that named value instead."
    return f"{base} No foundations value has this colour - confirm it belongs in foundations and export it there, or that this literal is wrong."


def main() -> int:
    """Check the real `crates/ui` tree against R120."""
    root = workspace.repo_root()
    if root is None:
        print("steg-ui-colours: not in a git repository", file=sys.stderr)
        return 2
    data_path = root / DESIGN_DATA
    data = foundations.read(data_path) if data_path.is_file() else foundations.Foundations(())
    index = foundations.hex_index(data)
    files = rust_files(root, UI_SRC) + rust_files(root, GALLERY)
    found = violations(files, index)
    for violation in found:
        print(format_violation(violation))
    return 1 if found else 0
