"""Write `crates/ui/src/foundations/generated.rs` from `crates/ui/design/foundations.json`
(TASK-1.93's plan; doc-17 R121).

A separate module from `steg_tools.foundations` (TASK-1.94's shared reader, merged to
`main` while this task's own review was still open) on purpose, confirmed rather than
assumed once it landed: that module's own `Foundations` deliberately reads only
`colours[]` and opacity names (its own docstring: "Sizes, type, motion, themes and the
focus ring are otherwise unread" - R120's colour-only AC#2 needed no more), where this
module has to turn *every* section of the file into Rust, including all of those. Its
`read()` offers nothing this module could call instead of its own `json.loads` without
losing most of the document, so no reconciliation was possible beyond confirming that -
stated here so a reviewer doesn't wonder why the merged shared reader goes unused.

The pure core (everything down to `main`) takes plain data and returns plain data - no
file I/O, no subprocess. `main` is the one thin shell: it reads the data file, renders,
and either writes `generated.rs` (plain) or (``--check``) rustfmts a fresh render into a
temp file and compares it, byte for byte, against what's committed.

Usage: steg-generate-foundations [--check]
"""

import json
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from steg_tools import workspace

REPO_ROOT_RELATIVE_DATA_FILE = "crates/ui/design/foundations.json"
REPO_ROOT_RELATIVE_GENERATED_FILE = "crates/ui/src/foundations/generated.rs"

# Metadata keys that never become a named entry (design/README.md: "where this was
# exported from", not a value).
_METADATA_KEYS = frozenset({"artifact_version", "artifact_url", "session_colour"})

# Fields that keep a group's "unit" sibling (such as "ms" or "px") but aren't
# themselves measured in it - a font weight, an em-relative letter-spacing ratio, or
# (TASK-1.136) the going-out order and the dimensionless fractions/strength beside
# motion.cursor-fade.shaded-dissolve's own "cycle". `_plain_field` consults this set
# for both a scalar field (below) and a list field (`order`'s own [i32; 6]) - named
# here once, rather than guessed per group, so a wrong guess is one line to fix.
_DIMENSIONLESS_FIELDS = frozenset(
    {
        "weight",
        "weight_selected",
        "letter_spacing",
        "order",
        "step_fraction",
        "medium_shade_strength",
        "phase_whole",
        "phase_going",
        "phase_empty",
        "phase_returning",
    }
)

# Within an otherwise-generic group, the one field that carries a colour and is
# rendered as an extra `Theme` method (by `render_sizes`/`render_focus_ring`) instead
# of a plain struct field.
_EXCLUDED_FIELD = {"status.ring": "track_colour", "track.layout": "fill", "focus_ring": "colour"}

_ANSI_NAMES = (
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
)


# ---------------------------------------------------------------------------
# identifiers
# ---------------------------------------------------------------------------

_NON_IDENT = re.compile(r"[^a-z0-9]+")


def identifier(name: str) -> str:
    """Derive a Rust identifier from one of the data file's own names.

    Lowercases `name` and collapses every run of non-alphanumeric characters (`.`,
    ` `, `-`) into a single `_`, trimming any leading/trailing `_`.
    """
    return _NON_IDENT.sub("_", name.lower()).strip("_")


def pascal_case(ident: str) -> str:
    """Turn a snake_case identifier into a PascalCase type name."""
    return "".join(word.capitalize() for word in ident.split("_"))


class IdentifierCollisionError(Exception):
    """Two distinct design names derived the same Rust identifier."""


def assign_identifiers(names: tuple[str, ...]) -> dict[str, str]:
    """Map each of `names` to its derived identifier.

    Raises:
        IdentifierCollisionError: two distinct names in `names` derive the same identifier.
    """
    owner_of: dict[str, str] = {}
    assigned: dict[str, str] = {}
    for name in names:
        ident = identifier(name)
        owner = owner_of.get(ident)
        if owner is not None and owner != name:
            raise IdentifierCollisionError(f'"{name}" and "{owner}" both derive the Rust identifier "{ident}"')
        owner_of[ident] = name
        assigned[name] = ident
    return assigned


# ---------------------------------------------------------------------------
# the three-way colour discrimination (design/README.md's own rule)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Ref:
    """A `{"ref": name}` pointer at another named entry in the file."""

    name: str


@dataclass(frozen=True)
class Session:
    """The bare string `"session"` - the active session's own colour, known only at
    runtime, never a fixed value here."""


def classify(value: object) -> str | Ref | Session:
    """Discriminate one colour-position value into a literal hex, a reference, or
    `"session"` - the only three shapes design/README.md documents.

    Raises:
        ValueError: `value` is none of the three documented shapes.
    """
    match value:
        case "session":
            return Session()
        case str() as hex_literal if hex_literal.startswith("#"):
            return hex_literal
        case {"ref": str() as name}:
            return Ref(name)
        case _:
            raise ValueError(f"not a colour value: {value!r}")


class Appearance(Enum):
    """Which of the design's two forms a value is being rendered for."""

    DARK = "dark"
    LIGHT = "light"


def bake_colour_for_appearance(value: object, appearance: Appearance, colours_index: dict[str, dict]) -> str:
    """Resolve one colour-position value to a literal hex for `appearance`, baking any
    ref to a `colours[]` entry. Used only where there is no `Theme`/`self` to delegate
    through - today, only the terminal themes' own fields, which are free functions,
    not `Theme` methods.

    Raises:
        ValueError: `value` resolves to `"session"` (every "session" field must be
            excluded before calling this), or a ref names an unknown colour.
    """
    match classify(value):
        case str() as hex_literal:
            return hex_literal
        case Ref(name):
            target = colours_index.get(name)
            if target is None:
                raise ValueError(f"ref to unknown colour {name!r}")
            return target[appearance.value]
        case Session():
            raise ValueError('"session" must be excluded before baking a colour')


def resolve_size_ref(value: object, index: dict[str, object]) -> float:
    """Resolve a plain (non-colour) value: a `{"ref": name}` to the numeric `value`
    field of the sized entry it names (`track.layout`'s `radius_outer`/`radius_inner`),
    or a bare number to itself - most size fields are plain literals, only a few are refs.

    Raises:
        ValueError: `value` is a ref naming something with no numeric `value` field, or
            is neither a ref nor a number.
    """
    match value:
        case {"ref": str() as name}:
            target = index.get(name)
            if not isinstance(target, dict) or "value" not in target:
                raise ValueError(f"ref target {name!r} has no plain numeric value")
            return target["value"]
        case int() | float() as number:
            return number
        case _:
            raise ValueError(f"not a plain sizeable value: {value!r}")


# ---------------------------------------------------------------------------
# design_entries: one name per addressable entry in the file, in file order
# ---------------------------------------------------------------------------


def design_entries(data: dict[str, object]) -> tuple[tuple[str, str, object], ...]:
    """Every (section, name, value) doc-17/AC#4 asks a reviewer to match - one per
    addressable entry in foundations.json, in file order.

    A list section (`colours`, `ramps`, ...) yields one entry per item, keyed by the
    item's own "name". A dict-of-groups section (`type`, `sizes` - every value itself
    an object) yields one entry per key. A lone object with its own "name" field
    (`tint`) yields that name; a lone object with none (`focus_ring`) yields the
    section's own top-level key instead.

    Raises:
        ValueError: a top-level section's value is none of those shapes.
    """
    entries: list[tuple[str, str, object]] = []
    for section, value in data.items():
        if section in _METADATA_KEYS:
            continue
        match value:
            case list() as items:
                entries.extend((section, item["name"], item) for item in items)
            case {"name": str() as name} as lone_named:
                entries.append((section, name, lone_named))
            case dict() as groups if groups and all(isinstance(v, dict) for v in groups.values()):
                entries.extend((section, key, item) for key, item in groups.items())
            case dict() as lone_unnamed:
                entries.append((section, section, lone_unnamed))
            case _:
                raise ValueError(f"unexpected top-level shape for {section!r}: {value!r}")
    return tuple(entries)


# ---------------------------------------------------------------------------
# rendering: Rust source text, one item (or a small cluster) per design name
# ---------------------------------------------------------------------------

# The "Design name" marker: exactly one per entry `design_entries` returns, on that
# entry's primary/public generated item - what the adapter test counts against
# `design_entries`' own count, for AC#4.


def _marker(name: str, use: str | None) -> str:
    doc = f'/// Design name: "{name}".'
    if use:
        doc += f" {use}"
    return doc


def _hex_to_int(hex_literal: str) -> int:
    return int(hex_literal.removeprefix("#"), 16)


def render_colours(data: dict[str, object]) -> tuple[str, list[str]]:
    """Render every `colours` entry's private dark/light functions and its `Theme`
    method, and every `colour_aliases` entry's `Theme` method - which delegates to the
    colour it names rather than duplicating its hex (no separate index needed: an
    alias's own `ref` field is already exactly the one-hop shape `classify` reads).
    """
    functions = []
    methods = []
    for colour in data.get("colours", []):
        name = colour["name"]
        ident = identifier(name)
        functions.append(f"fn {ident}_dark() -> Hsla {{\n    hsla_from_hex(0x{_hex_to_int(colour['dark']):06x})\n}}")
        functions.append(
            f"fn {ident}_light() -> Hsla {{\n    hsla_from_hex(0x{_hex_to_int(colour['light']):06x})\n}}"
        )
        methods.append(
            f"{_marker(name, colour.get('use'))}\n"
            f"    pub fn {ident}(&self) -> Hsla {{\n"
            f"        match self.appearance {{\n"
            f"            Appearance::Dark => {ident}_dark(),\n"
            f"            Appearance::Light => {ident}_light(),\n"
            f"        }}\n"
            f"    }}"
        )
    for alias in data.get("colour_aliases", []):
        name = alias["name"]
        ident = identifier(name)
        match classify(alias["ref"]):
            case Ref(target_name):
                target_ident = identifier(target_name)
            case other:
                raise ValueError(f"colour_aliases[{name!r}].ref is expected to be a ref: {other!r}")
        methods.append(
            f"{_marker(name, alias.get('use'))}\n"
            f"    pub fn {ident}(&self) -> Hsla {{\n"
            f"        self.{target_ident}()\n"
            f"    }}"
        )
    return "\n\n".join(functions), methods


def render_opacities(data: dict[str, object]) -> list[str]:
    """Render every `opacities` entry's `Theme` method - a light/dark pair of plain floats."""
    methods = []
    for opacity in data.get("opacities", []):
        name = opacity["name"]
        ident = identifier(name)
        methods.append(
            f"{_marker(name, opacity.get('use'))}\n"
            f"    pub fn {ident}(&self) -> f32 {{\n"
            f"        match self.appearance {{\n"
            f"            Appearance::Dark => {float(opacity['dark'])},\n"
            f"            Appearance::Light => {float(opacity['light'])},\n"
            f"        }}\n"
            f"    }}"
        )
    return methods


def render_tint(data: dict[str, object]) -> list[str]:
    """Render `tint`'s `Theme` method: its colour is always `"session"`, its opacity plain."""
    tint = data["tint"]
    if classify(tint["colour"]) != Session():
        raise ValueError('tint.colour is expected to be "session"')
    return [
        f"{_marker(tint['name'], tint.get('use'))}\n"
        f"    pub fn tint(&self) -> (Hsla, f32) {{\n"
        f"        (self.session, {float(tint['opacity'])})\n"
        f"    }}"
    ]


def render_ramps(data: dict[str, object]) -> list[str]:
    """Render every `ramps` entry's `Theme` method: each stop delegates to the colour
    it names, and the entry's `caption_opacity`, when it has one, rides along in the
    same returned value rather than becoming a separate design name."""
    methods = []
    for ramp in data.get("ramps", []):
        name = ramp["name"]
        ident = identifier(name)
        stop_entries = []
        for stop in ramp["stops"]:
            match classify(stop["colour"]):
                case Ref(target_name):
                    stop_entries.append(f"({float(stop['stop'])}, self.{identifier(target_name)}())")
                case other:
                    raise ValueError(f"ramp stop colour is expected to be a ref: {other!r}")
        stops_text = ", ".join(stop_entries)
        if "caption_opacity" in ramp:
            methods.append(
                f"{_marker(name, ramp.get('use'))}\n"
                f"    pub fn {ident}(&self) -> RampValue {{\n"
                f"        RampValue {{\n"
                f"            stops: vec![{stops_text}],\n"
                f"            caption_opacity: match self.appearance {{\n"
                f"                Appearance::Dark => {float(ramp['caption_opacity']['dark'])},\n"
                f"                Appearance::Light => {float(ramp['caption_opacity']['light'])},\n"
                f"            }},\n"
                f"        }}\n"
                f"    }}"
            )
        else:
            methods.append(
                f"{_marker(name, ramp.get('use'))}\n"
                f"    pub fn {ident}(&self) -> Vec<(f32, Hsla)> {{\n"
                f"        vec![{stops_text}]\n"
                f"    }}"
            )
    return methods


def render_scrim(scrim: dict[str, object]) -> str:
    """Render `scrim`'s `Theme` method. Each branch's own colour either delegates to
    the `Theme` method it names (`scrim.light.colour`'s ref to `text` reads
    `self.text()` inside the *light* match arm specifically - proving it reaches
    `text`'s light form, not baking a value that could silently drift from which arm
    calls it) or bakes a literal (`scrim.dark.colour`, which names nothing to delegate to).

    Raises:
        ValueError: a branch's colour resolves to `"session"` (not expected of scrim).
    """
    arms = []
    for appearance in Appearance:
        branch = scrim[appearance.value]
        match classify(branch["colour"]):
            case Ref(target_name):
                colour_expr = f"self.{identifier(target_name)}()"
            case str() as hex_literal:
                colour_expr = f"hsla_from_hex(0x{_hex_to_int(hex_literal):06x})"
            case Session():
                raise ValueError('scrim is not expected to resolve to "session"')
        variant = "Dark" if appearance is Appearance.DARK else "Light"
        arms.append(
            f"            Appearance::{variant} => "
            f"ScrimValue {{ colour: {colour_expr}, opacity: {float(branch['opacity'])} }},"
        )
    return (
        f'{_marker("scrim", scrim.get("use"))}\n'
        "    pub fn scrim(&self) -> ScrimValue {\n"
        "        match self.appearance {\n" + "\n".join(arms) + "\n"
        "        }\n"
        "    }"
    )


def _format_number(number: float) -> str:
    """Render a number the way a person reading the design would write it - no
    trailing `.0` on a whole number."""
    return str(int(number)) if number == int(number) else str(number)


def _plain_field(
    group: dict[str, object], key: str, value: object, index: dict[str, object]
) -> tuple[str, str, str]:
    """Render one non-colour field of an otherwise-plain group to (rust_type,
    initialiser, display) - `display` is the field's value in the design's own words
    (`"232px"`, `"system"`), never Rust syntax; it's what `_DESCRIPTION` reads back,
    not `{:?}` on the generated struct (a reviewer matches the design's page, not Rust).

    Raises:
        ValueError: `value` is a shape with no sensible plain Rust rendering.
    """
    unit = group.get("unit")
    match value:
        case list() as items if items and isinstance(items[0], str):
            display = ", ".join(items)
            return "&'static [&'static str]", "&[" + ", ".join(f'"{item}"' for item in items) + "]", display
        case list() as items:
            resolved = [resolve_size_ref(item, index) for item in items]
            # A field in _DIMENSIONLESS_FIELDS (TASK-1.136: motion.cursor-fade.
            # shaded-dissolve's "order" beside its own group's "ms"-measured "cycle")
            # keeps the group's unit sibling without being measured in it - the same
            # exemption the scalar branch below already makes, reused here rather
            # than invented twice.
            effective_unit = None if key in _DIMENSIONLESS_FIELDS else unit
            if effective_unit == "px":
                display = ", ".join(f"{_format_number(n)}px" for n in resolved)
                return f"[Pixels; {len(items)}]", "[" + ", ".join(f"px({float(n)})" for n in resolved) + "]", display
            if all(n == int(n) for n in resolved):
                display = ", ".join(_format_number(n) for n in resolved)
                return f"[i32; {len(items)}]", "[" + ", ".join(str(int(n)) for n in resolved) + "]", display
            display = ", ".join(f"{_format_number(n)}{effective_unit or ''}" for n in resolved)
            return f"[f32; {len(items)}]", "[" + ", ".join(str(float(n)) for n in resolved) + "]", display
        case str() as text:
            return "&'static str", f'"{text}"', text
        case {"ref": str()}:
            number = resolve_size_ref(value, index)
            display = f"{_format_number(number)}{unit or ''}"
            if unit == "px":
                return "Pixels", f"px({float(number)})", display
            return "f32", str(float(number)), display
        case int() | float() as number:
            if key in _DIMENSIONLESS_FIELDS:
                return "f32", str(float(number)), _format_number(number)
            if unit == "ms":
                return (
                    "std::time::Duration",
                    f"std::time::Duration::from_millis({int(number)})",
                    f"{_format_number(number)}ms",
                )
            if unit == "px":
                return "Pixels", f"px({float(number)})", f"{_format_number(number)}px"
            return "f32", str(float(number)), _format_number(number)
        case _:
            raise ValueError(f"unexpected plain field {key!r}: {value!r}")


def render_plain_group(name: str, group: dict[str, object], index: dict[str, object]) -> str:
    """Render one `type`/`sizes`/`motion`/`focus_ring` entry with no colour of its own
    (or whose one colour field `_EXCLUDED_FIELD` names, rendered separately by the
    caller) as a `pub struct` + `pub const`, or - with exactly one field left - a bare
    `pub const` of that field's own type; either way, also a `pub const
    <IDENT>_DESCRIPTION: &str` in the design's own words (the gallery's "Other named
    values" section reads this, not `{:?}` on the struct, since a Rust debug dump of
    nested braces and field names isn't what a reviewer matches against the design's
    own page - team lead's review of this task's first gallery capture).

    Raises:
        ValueError: nothing is left to render once metadata and the excluded field are
            skipped, or one of its fields is a shape `_plain_field` doesn't recognise.
    """
    ident = identifier(name)
    excluded = _EXCLUDED_FIELD.get(name)
    fields = []
    for key, value in group.items():
        if key in ("name", "use") or key == "unit" or key.endswith("_unit") or key == excluded:
            continue
        rust_type, initialiser, display = _plain_field(group, key, value, index)
        fields.append((key, rust_type, initialiser, display))
    if not fields:
        raise ValueError(f"group {name!r} has no plain fields to render")
    marker = _marker(name, group.get("use"))
    if len(fields) == 1:
        _, rust_type, initialiser, display = fields[0]
        # A `const`'s own type gets an implicit `'static` on every reference in it
        # (unlike a struct field's, which needs it spelled out) - clippy's
        # `redundant_static_lifetimes` refuses writing it anyway, so this is the one
        # place `_plain_field`'s otherwise-reusable `&'static ...` type isn't reused
        # verbatim.
        const_type = rust_type.replace("'static ", "")
        description = f'pub const {ident.upper()}_DESCRIPTION: &str = "{display}";'
        return f"{marker}\npub const {ident.upper()}: {const_type} = {initialiser};\n\n{description}"
    struct_name = pascal_case(ident)
    struct_fields = "\n".join(f"    pub {field_name}: {rust_type}," for field_name, rust_type, _, _ in fields)
    const_fields = "\n".join(f"    {field_name}: {initialiser}," for field_name, _, initialiser, _ in fields)
    display_text = ", ".join(f"{field_name} {display}" for field_name, _, _, display in fields)
    description = f'pub const {ident.upper()}_DESCRIPTION: &str = "{display_text}";'
    return (
        f"#[derive(Debug, Clone, Copy)]\npub struct {struct_name} {{\n{struct_fields}\n}}\n\n"
        f"{marker}\npub const {ident.upper()}: {struct_name} = {struct_name} {{\n{const_fields}\n}};\n\n"
        f"{description}"
    )


def render_sizes(data: dict[str, object]) -> tuple[str, list[str]]:
    """Render every `sizes` entry: the generic plain-group path for most of them, and
    `radius.round`, `scrim`, `status.ring` and `track.layout` by hand - each for its
    own reason (TASK-1.93's plan).

    Returns the Rust text for the plain items, and the extra `Theme` method bodies the
    hand-rendered groups need.

    Raises:
        ValueError: `status.ring.track_colour` or `track.layout.fill.colour` isn't a
            ref (both are expected to be, per the data file today).
    """
    sizes: dict[str, dict] = data["sizes"]
    items = []
    methods = []
    for name, group in sizes.items():
        if name == "radius.round":
            # No px value in the data at all (design/README.md: "not a design-named
            # value in px"). A conventionally large pixel radius - the data file's own
            # comment names the same CSS trick - renders identically to the other
            # three radii (one type for all four corners, applied the same way by a
            # component), not a separate marker type: gpui 0.2.2's `corner_radii`
            # takes `Pixels` with no "fully round" primitive of its own
            # (`AbsoluteLength` is `Pixels` or `Rems` only), and a radius at or past
            # half of whatever it rings paints fully round regardless of the exact
            # number used to get there.
            items.append(f"{_marker(name, group.get('use'))}\npub const RADIUS_ROUND: Pixels = px(999.0);")
        elif name == "scrim":
            methods.append(render_scrim(group))
        elif name == "status.ring":
            match classify(group["track_colour"]):
                case Ref(target_name):
                    methods.append(
                        f"pub fn status_ring_track(&self) -> Hsla {{\n        self.{identifier(target_name)}()\n    }}"
                    )
                case other:
                    raise ValueError(f"status.ring.track_colour is expected to be a ref: {other!r}")
            items.append(render_plain_group(name, group, sizes))
        elif name == "track.layout":
            match classify(group["fill"]["colour"]):
                case Ref(target_name):
                    methods.append(
                        f"pub fn track_layout_fill(&self) -> (Hsla, f32) {{\n        self.{identifier(target_name)}()\n    }}"
                    )
                case other:
                    raise ValueError(f"track.layout.fill.colour is expected to be a ref: {other!r}")
            items.append(render_plain_group(name, group, sizes))
        else:
            items.append(render_plain_group(name, group, sizes))
    return "\n\n".join(items), methods


def render_focus_ring(data: dict[str, object]) -> tuple[str, str]:
    """Render `focus_ring`'s plain geometry as a struct/const, and its one colour
    field - always `"session"`, never a named colour - as its own `Theme` method.

    Raises:
        ValueError: `focus_ring.colour` isn't `"session"`.
    """
    focus_ring = data["focus_ring"]
    if classify(focus_ring["colour"]) != Session():
        raise ValueError('focus_ring.colour is expected to be "session"')
    item = render_plain_group("focus_ring", focus_ring, {})
    method = "pub fn focus_ring_colour(&self) -> Hsla {\n        self.session\n    }"
    return item, method


def render_type(data: dict[str, object]) -> str:
    """Render every `type` entry through the same generic plain-group path `sizes`
    mostly uses - `type` has no colours and no refs, so nothing else is needed."""
    return "\n\n".join(render_plain_group(name, group, {}) for name, group in data.get("type", {}).items())


def render_motion(data: dict[str, object]) -> str:
    """Render every `motion` entry (today, one: `motion.status-fade`) through the same
    generic plain-group path - its `"unit": "ms"` is what makes `_plain_field` choose
    `Duration` over `Pixels`."""
    return "\n\n".join(render_plain_group(entry["name"], entry, {}) for entry in data.get("motion", []))


def render_themes(data: dict[str, object], colours_index: dict[str, dict]) -> str:
    """Render the two terminal themes as free functions taking the session colour -
    free functions, not `Theme` methods, because each is a fixed, flat colour set (not
    toggled by the app's own appearance) and the data file's one runtime value in
    them, `cursor: "session"`, has to enter the same way `Theme::new` takes it.

    Raises:
        ValueError: a theme's `cursor` isn't `"session"`.
    """
    structs = (
        "#[derive(Debug, Clone, Copy)]\npub struct AnsiColours {\n"
        + "\n".join(f"    pub {ansi}: Hsla," for ansi in _ANSI_NAMES)
        + "\n}\n\n"
        "#[derive(Debug, Clone, Copy)]\npub struct TerminalTheme {\n"
        "    pub bg: Hsla,\n"
        "    pub bg_inactive: Hsla,\n"
        "    pub fg: Hsla,\n"
        "    pub cursor: Hsla,\n"
        "    pub selection: Hsla,\n"
        "    pub ansi: AnsiColours,\n"
        "}"
    )
    functions = []
    for theme in data.get("themes", []):
        name = theme["name"]
        ident = identifier(name)
        if classify(theme["cursor"]) != Session():
            raise ValueError(f'{name!r}\'s cursor is expected to be "session"')
        # "steg dark"/"steg light" - every ref in a theme (bg_inactive) resolves
        # against *that* theme's own appearance, never the other one.
        appearance = Appearance.DARK if "dark" in name else Appearance.LIGHT
        bg = bake_colour_for_appearance(theme["bg"], appearance, colours_index)
        bg_inactive = bake_colour_for_appearance(theme["bg_inactive"], appearance, colours_index)
        fg = bake_colour_for_appearance(theme["fg"], appearance, colours_index)
        selection = bake_colour_for_appearance(theme["selection"], appearance, colours_index)
        ansi_fields = "\n".join(
            f"            {ansi}: hsla_from_hex(0x"
            f"{_hex_to_int(bake_colour_for_appearance(theme['ansi'][ansi], appearance, colours_index)):06x}),"
            for ansi in _ANSI_NAMES
        )
        functions.append(
            f"{_marker(name, None)}\n"
            f"pub fn {ident}(session: Hsla) -> TerminalTheme {{\n"
            f"    TerminalTheme {{\n"
            f"        bg: hsla_from_hex(0x{_hex_to_int(bg):06x}),\n"
            f"        bg_inactive: hsla_from_hex(0x{_hex_to_int(bg_inactive):06x}),\n"
            f"        fg: hsla_from_hex(0x{_hex_to_int(fg):06x}),\n"
            f"        cursor: session,\n"
            f"        selection: hsla_from_hex(0x{_hex_to_int(selection):06x}),\n"
            f"        ansi: AnsiColours {{\n{ansi_fields}\n        }},\n"
            f"    }}\n"
            f"}}"
        )
    return structs + "\n\n" + "\n\n".join(functions)


_HEADER = (
    "//! Generated by `tools/src/steg_tools/foundations_generate.py` from\n"
    "//! `crates/ui/design/foundations.json` (artifact version `{artifact_version}`). Never\n"
    "//! edited by hand (R121) - a value changes by changing the design and exporting again,\n"
    "//! then running `just generate-foundations`."
)

_IMPORTS = "use gpui::{Hsla, Pixels, px};\n\nuse super::{Appearance, Theme};"


def render(data: dict[str, object]) -> str:
    """Render the whole of `generated.rs` from the parsed data file.

    Raises:
        IdentifierCollisionError: two distinct design names derive the same identifier.
        ValueError: the data file doesn't match a shape design/README.md documents.
    """
    assign_identifiers(tuple(name for _, name, _ in design_entries(data)))  # fail fast on any collision

    colours_index = {colour["name"]: colour for colour in data["colours"]}
    colour_functions, colour_methods = render_colours(data)
    opacity_methods = render_opacities(data)
    tint_methods = render_tint(data)
    ramp_methods = render_ramps(data)
    sizes_items, sizes_extra_methods = render_sizes(data)
    focus_ring_item, focus_ring_method = render_focus_ring(data)

    theme_methods = (
        colour_methods + opacity_methods + tint_methods + ramp_methods + sizes_extra_methods + [focus_ring_method]
    )
    theme_impl = "impl Theme {\n" + "\n\n".join(theme_methods) + "\n}"

    sections = [
        _HEADER.format(artifact_version=data["artifact_version"]),
        _IMPORTS,
        "fn hsla_from_hex(hex: u32) -> Hsla {\n    gpui::rgb(hex).into()\n}",
        # No Copy: `Vec` isn't.
        "#[derive(Debug, Clone)]\npub struct RampValue {\n    pub stops: Vec<(f32, Hsla)>,\n    pub caption_opacity: f32,\n}",
        "#[derive(Debug, Clone, Copy)]\npub struct ScrimValue {\n    pub colour: Hsla,\n    pub opacity: f32,\n}",
        colour_functions,
        theme_impl,
        render_type(data),
        sizes_items,
        focus_ring_item,
        render_motion(data),
        render_themes(data, colours_index),
    ]
    return "\n\n".join(section for section in sections if section) + "\n"


# ---------------------------------------------------------------------------
# the regenerate-and-diff logic `check-foundations` uses
# ---------------------------------------------------------------------------


def is_stale(rendered: str, existing: str) -> bool:
    """Say whether freshly rendered (and, for `--check`, rustfmt'd) Rust text differs
    from what's committed, byte for byte."""
    return rendered != existing


def staleness_message() -> str:
    """The message `check-foundations` reports on a mismatch, naming the fix."""
    return (
        f"{REPO_ROOT_RELATIVE_GENERATED_FILE} does not match {REPO_ROOT_RELATIVE_DATA_FILE} - "
        "run `just generate-foundations` and commit the result."
    )


# ---------------------------------------------------------------------------
# the shell
# ---------------------------------------------------------------------------


def _rustfmt(path: Path) -> None:
    """Run `rustfmt` on `path` in place.

    Raises:
        RuntimeError: `rustfmt` isn't on `PATH`, or fails.
    """
    done = subprocess.run(["rustfmt", "--edition", "2024", str(path)], capture_output=True, text=True)
    if done.returncode != 0:
        raise RuntimeError(f"rustfmt failed on {path}:\n{done.stderr}")


def main(argv: list[str] | None = None) -> int:
    """Generate `generated.rs` from the data file, or (``--check``) rustfmt a fresh
    render into a temp file and compare it against what's committed.

    `argv` defaults to the real command line (`sys.argv[1:]`); a test passes its own
    list instead.
    """
    args = sys.argv[1:] if argv is None else argv
    check_only = "--check" in args
    root = workspace.repo_root()
    if root is None:
        print("steg-generate-foundations: not in a git repository", file=sys.stderr)
        return 2
    data_path = root / REPO_ROOT_RELATIVE_DATA_FILE
    generated_path = root / REPO_ROOT_RELATIVE_GENERATED_FILE
    data = json.loads(data_path.read_text())
    rendered = render(data)

    if not check_only:
        generated_path.write_text(rendered)
        print(f"ok      wrote {REPO_ROOT_RELATIVE_GENERATED_FILE} - run rustfmt before committing")
        return 0

    with tempfile.NamedTemporaryFile(suffix=".rs", mode="w", delete=False) as handle:
        handle.write(rendered)
        temp_path = Path(handle.name)
    try:
        _rustfmt(temp_path)
        freshly_formatted = temp_path.read_text()
    finally:
        temp_path.unlink(missing_ok=True)

    existing = generated_path.read_text() if generated_path.exists() else ""
    if is_stale(freshly_formatted, existing):
        print(staleness_message(), file=sys.stderr)
        return 1
    print(f"ok      {REPO_ROOT_RELATIVE_GENERATED_FILE} matches {REPO_ROOT_RELATIVE_DATA_FILE}")
    return 0
