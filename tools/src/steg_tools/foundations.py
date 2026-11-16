"""Read `crates/ui/design/foundations.json`'s named colours, and build a lookup from a
hex value back to every name that has it.

Shared by `ui_colours` (R120, this task) - and meant for TASK-1.96's check of the data
file against the design's saved pages to reuse, rather than writing its own JSON reader.
Reads only `colours[]`: `colour_aliases[]` (`blocked`, `done`, `error`) always resolves
to the same hex as the colour it references (`design/README.md`'s own description of
how a reference is written), so a match message naming the base colour is enough - this
module doesn't also resolve aliases, a scoping cut, not an oversight.
"""

import json
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class NamedColour:
    """One `colours[]` entry: its name, and its dark and light hex forms."""

    name: str
    dark: str
    light: str


@dataclass(frozen=True)
class Foundations:
    """The part of `foundations.json` this package reads today: the named colours, and
    the names of every value that is an opacity or carries one (`opacity_names`).

    Sizes, type, motion, themes and the focus ring are otherwise unread - R120's checks
    for those kinds of literal (size, radius, duration, font) are not this task's AC#2,
    which names colour only - a later task's, if wanted, not grown into this one.
    """

    colours: tuple[NamedColour, ...]
    opacity_names: tuple[str, ...] = ()


def normalise_hex(value: str) -> str:
    """Strip a leading `#` or `0x`/`0X` and lowercase, so `#1C1B21`, `0x1c1b21` and
    `1c1b21` all compare equal."""
    return value.removeprefix("#").removeprefix("0x").removeprefix("0X").lower()


def opacity_names_from_document(document: dict[str, object]) -> tuple[str, ...]:
    """Every named value in the raw parsed JSON that is an opacity, or carries one.

    Not a generic walk: the file has no one field common to every shape that holds an
    opacity, so this is the fixed list of the specific places `design/README.md` itself
    describes as carrying one - a fifth shape added to the design later needs a new
    line here too, named so that need is visible rather than a silent miss:

    - every `opacities[].name` (a value that *is* an opacity, nothing else - `faint` today).
    - `tint.name`, because `tint` carries its own `opacity` field directly.
    - any `ramps[].name` whose entry has a `caption_opacity` field (only `ramp` does -
      `ramp.extended` has stops but no caption, so it contributes nothing).
    - the literal key `scrim` under `sizes`, because each of its `dark`/`light` forms
      carries its own `opacity` field - `scrim` has no `name` field of its own (it's a
      key in `sizes`, not an entry in a named list), so this reads the key itself.
    """
    names = [entry["name"] for entry in document.get("opacities", [])]
    tint = document.get("tint")
    if isinstance(tint, dict) and "opacity" in tint and "name" in tint:
        names.append(tint["name"])
    for ramp in document.get("ramps", []):
        if "caption_opacity" in ramp:
            names.append(ramp["name"])
    sizes = document.get("sizes")
    if isinstance(sizes, dict) and "scrim" in sizes:
        names.append("scrim")
    return tuple(names)


def read(path: Path) -> Foundations:
    """Read `foundations.json`'s named colours and opacity names.

    Raises:
        OSError: `path` can't be read.
        json.JSONDecodeError: `path` isn't valid JSON.
        KeyError: a `colours[]` entry is missing `name`, `dark` or `light`.
    """
    document = json.loads(path.read_text())
    colours = tuple(NamedColour(entry["name"], entry["dark"], entry["light"]) for entry in document["colours"])
    return Foundations(colours, opacity_names_from_document(document))


def hex_index(foundations: Foundations) -> dict[str, tuple[tuple[str, str], ...]]:
    """Map each normalised hex value to every `(name, "dark"|"light")` pair that has it.

    Two names can share a hex in one form and differ in the other - the data file's own
    `outline` and `track` entries coincide in dark (`#3d3b45`) and differ in light - and
    both come back under the same key, never merged, so a caller can name both.
    """
    found: dict[str, list[tuple[str, str]]] = {}
    for colour in foundations.colours:
        for value, form in ((colour.dark, "dark"), (colour.light, "light")):
            found.setdefault(normalise_hex(value), []).append((colour.name, form))
    return {key: tuple(pairs) for key, pairs in found.items()}
