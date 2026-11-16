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
float triple with no distinguishing call or struct name; a colour literal written
*inside* a macro's arguments, for the same reason `ui_levels` can't see a cross-level
path there - the parser hands a macro invocation's body back as an opaque token tree. A
named constant defined outside foundations and merely *used* elsewhere is not a blind
spot - that constant's own definition is a literal outside foundations too, and is
caught there, at its one true source.

`rgb`/`rgba`'s own argument, when it is itself a hex literal, is read back out and
matched against foundations the same way a bare hex is - `rgb(0x3d3b45)` names the same
colour as a bare `0x3d3b45` would, and must match the same names. A call whose argument
isn't itself a literal (a variable, an expression) is still refused as a colour
constructor call outside foundations - there's simply no name to suggest for it.

An 8-digit value's trailing two digits are alpha. When alpha is `ff` (fully opaque) or
absent (a 6-digit value has none to begin with), it's dropped before matching and the
message reads as if it were never there - `rgba(0x3d3b45ff)` matches the same names
`rgb(0x3d3b45)` does. Any other alpha is a genuinely different colour from the opaque
one it's compared against - `rgba(0x1c1b2180)` is `surface.raised` at roughly half
strength, not `surface.raised` itself, and saying "use that named value instead" would
be wrong: the opaque named colour isn't a fix for a translucent literal. The message
instead says what it actually is (the matching colour's name, and the alpha byte as a
rounded percentage) and that foundations holds opacity as named values too - the names
it offers are read from the data file itself
(`foundations.opacity_names_from_document`), not typed here, so the message can never
name an opacity the design no longer has - so the fix is naming both the colour and an
opacity, or adding a new named value to the design, never the bare literal.

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
    """One colour literal found in a file: its line, its text, its normalised RGB hex
    value with alpha dropped - a named-constructor call with no hex argument, or a bare
    struct literal, has none - and the alpha byte as a rounded percentage, `None` when
    it's fully opaque (absent, or `ff`)."""

    line: int
    text: str
    hex_value: str | None
    alpha_percent: int | None = None


@dataclass(frozen=True)
class Violation:
    """One colour literal outside foundations: where it is, every foundations value
    (name, "dark"|"light") whose hex it matches - empty when none match - and, when it
    carries a genuine alpha, that alpha as a rounded percentage (`None` means fully
    opaque, the ordinary case)."""

    path: str
    line: int
    text: str
    matches: tuple[tuple[str, str], ...]
    alpha_percent: int | None = None


def is_exempt(path: str) -> bool:
    """Say whether a literal in this file is allowed: see `EXEMPT_PREFIXES` above."""
    return any(path == prefix or path.startswith(f"{prefix}/") for prefix in EXEMPT_PREFIXES)


def _line(node: SgNode) -> int:
    """A node's 1-indexed starting line."""
    return node.range().start.line + 1


# Of COLOUR_CONSTRUCTORS, only these two take a hex value as their argument (gpui's
# `src/color.rs`: `rgb(hex: u32)`, `rgba(hex: u32)`) - every other name there takes
# plain floats or nothing, so there is no hex to read back out of a call to it.
HEX_ARGUMENT_CONSTRUCTORS = frozenset({"rgb", "rgba"})


def _is_colour_constructor_call(node: SgNode) -> bool:
    """Say whether a `call_expression` node calls one of `COLOUR_CONSTRUCTORS`, however qualified."""
    callee = node.field("function")
    return callee is not None and callee.text().rsplit("::", 1)[-1] in COLOUR_CONSTRUCTORS


def _alpha_percent(normalised_hex: str) -> int | None:
    """The alpha byte of an 8-digit normalised hex value, as a rounded percentage -
    `None` when there's no alpha digit (a 6-digit value) or it's `ff` (fully opaque):
    both read, to a match, as if there were no alpha at all."""
    if len(normalised_hex) < 8:
        return None
    alpha_byte = normalised_hex[6:8]
    if alpha_byte == "ff":
        return None
    return round(int(alpha_byte, 16) / 255 * 100)


def _rgb_and_alpha(text: str) -> tuple[str, int | None]:
    """A hex literal's text, normalised to its RGB portion for matching (lowercase, no
    `#`/`0x` prefix, alpha digits dropped) and its alpha as a rounded percentage - see
    `_alpha_percent`."""
    normalised = foundations.normalise_hex(text)
    return normalised[:6], _alpha_percent(normalised)


def _hex_argument(node: SgNode) -> tuple[str, int | None] | None:
    """The `(rgb_hex, alpha_percent)` a `rgb`/`rgba` call's own argument names, when
    that argument is itself a hex literal - `None` for any other constructor (none of
    them take a hex argument at all) or for an argument that isn't a plain literal (a
    variable, an expression): that call is still refused as a colour constructor
    outside foundations, just with no name to suggest for it."""
    callee = node.field("function")
    if callee is None or callee.text().rsplit("::", 1)[-1] not in HEX_ARGUMENT_CONSTRUCTORS:
        return None
    arguments = node.field("arguments")
    if arguments is None:
        return None
    parts = arguments.named_children()
    if len(parts) != 1:
        return None
    argument = parts[0]
    if argument.kind() == "integer_literal" and HEX_INT_RE.match(argument.text()):
        return _rgb_and_alpha(argument.text())
    return None


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
            extracted = _hex_argument(node)
            hex_value, alpha_percent = extracted if extracted is not None else (None, None)
            found.append(Literal(_line(node), node.text(), hex_value, alpha_percent))
    for node in root.find_all(kind="integer_literal"):
        if HEX_INT_RE.match(node.text()) and not _inside_colour_constructor_call(node):
            hex_value, alpha_percent = _rgb_and_alpha(node.text())
            found.append(Literal(_line(node), node.text(), hex_value, alpha_percent))
    for node in root.find_all(kind="string_literal"):
        if HEX_STRING_RE.match(node.text()):
            hex_value, alpha_percent = _rgb_and_alpha(node.text().strip('"'))
            found.append(Literal(_line(node), node.text(), hex_value, alpha_percent))
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
            found.append(Violation(file.path, literal.line, literal.text, matches, literal.alpha_percent))
    return tuple(sorted(found, key=lambda violation: (violation.path, violation.line)))


def format_violation(violation: Violation, opacity_names: tuple[str, ...]) -> str:
    """Report one violation, naming the matching foundations value where there is one, and R120's own rule.

    A match with a genuine (non-opaque) alpha gets its own wording: the literal is a
    translucent version of a foundations colour, not that colour itself, so "use that
    named value instead" would be wrong advice - the fix is naming the colour *and* an
    opacity. `opacity_names` is read from the data file itself
    (`foundations.opacity_names_from_document`), not typed here, so this message can
    never name an opacity the design no longer has.
    """
    base = f"{violation.path}:{violation.line}: `{violation.text}` is a colour literal, which doc-17 R120 refuses outside foundations."
    if not violation.matches:
        return f"{base} No foundations value has this colour - confirm it belongs in foundations and export it there, or that this literal is wrong."
    names = ", ".join(f"{name} ({form})" for name, form in sorted(violation.matches))
    if violation.alpha_percent is not None:
        return (
            f"{base} It is the colour of {names} at {violation.alpha_percent}% opacity, not that colour "
            f"itself - foundations holds opacity as its own named values too ({', '.join(opacity_names)}), "
            f"so name both the colour and an opacity, or add a new named value to the design - never the "
            f"bare literal."
        )
    return f"{base} This value matches {names} - use that named value instead."


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
        print(format_violation(violation, data.opacity_names))
    return 1 if found else 0
