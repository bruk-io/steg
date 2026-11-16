"""Refuse a just recipe whose name doesn't start with a verb in the imperative.

A recipe is something you tell the repository to do, so its name should say the
action: `just test`, `just check-boundaries`. ALLOWED_VERBS below is the short list
this check allows; extend it there when a recipe is genuinely a new action, never to
skip the check. A private recipe (just's own `[private]` attribute - today only
`default`, the bare `just`'s reserved entry point) is exempt: the rule is about the
word a person types, and nobody types that one by name.

Usage: steg-recipes
"""

import sys
from dataclasses import dataclass

from steg_tools import workspace

# Extended by editing this set, never by skipping the check below.
# "capture" (TASK-1.91): `capture-gallery` writes the gallery's rendered window to an
# image file - a genuinely new action, distinct from `build` (compiles, never runs
# anything) and `run` (opens a window for a person, produces no file).
# "generate" (TASK-1.93): `generate-foundations` writes crates/ui/src/foundations/
# generated.rs from the design data file - distinct from `build` (compiles existing
# source, writes nothing under src/) and from `check-foundations` (already legal
# under "check", reads but never writes).
ALLOWED_VERBS = frozenset(
    {"check", "test", "build", "fmt", "lint", "run", "setup", "deny", "show", "capture", "generate"}
)


@dataclass(frozen=True)
class Recipe:
    """One just recipe: its name, and whether it is `[private]`."""

    name: str
    private: bool


def first_word(name: str) -> str:
    """Take the leading `-`-separated segment of a recipe name, the word the verb rule judges."""
    return name.split("-", 1)[0]


def recipes_from_dump(dump: dict[str, object]) -> tuple[Recipe, ...]:
    """Read the recipes `just --dump --dump-format json` reports into plain data."""
    recipes = dump["recipes"]  # type: ignore[index]
    return tuple(Recipe(name, bool(info["private"])) for name, info in recipes.items())  # type: ignore[union-attr]


def offenders(recipes: tuple[Recipe, ...], verbs: frozenset[str]) -> tuple[Recipe, ...]:
    """Say which public recipes don't start with a verb on the allowed list.

    A private recipe is never an offender, whatever its name: nobody types it by name.
    """
    return tuple(recipe for recipe in recipes if not recipe.private and first_word(recipe.name) not in verbs)


def format_offender(recipe: Recipe) -> str:
    """Report one offending recipe, naming the word, the file to extend, and the reason."""
    return (
        f"`just {recipe.name}` isn't named with a verb: '{first_word(recipe.name)}' isn't on the allowed list in "
        f"tools/src/steg_tools/recipes.py. A recipe is something you tell the repository to do, so its name starts "
        f"with the action, such as `test-all` or `check-boundaries`. Extend the list there if this is genuinely a "
        f"new verb; never skip this check."
    )


def main() -> int:
    """Check the real justfile's recipe names against ALLOWED_VERBS."""
    root = workspace.repo_root()
    if root is None:
        print("steg-recipes: not in a git repository", file=sys.stderr)
        return 2
    dump = workspace.just_dump(root)
    if dump is None:
        print("steg-recipes: just --dump failed", file=sys.stderr)
        return 1
    found = offenders(recipes_from_dump(dump), ALLOWED_VERBS)
    for recipe in found:
        print(format_offender(recipe))
    return 1 if found else 0
