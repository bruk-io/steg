"""Refuse a crate dependency that crosses the boundaries in .claude/rules/architecture.md.

Each restricted crate's permitted dependencies - workspace and external alike - are
written down once in boundaries.toml. A crate with no entry there is unrestricted.
Nobody chooses the tests by hand and nobody keeps the boundary in their head either:
`just check-boundaries` fails on any dependency a restricted crate has that isn't on its list.

Usage: steg-boundaries
"""

import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

from steg_tools import workspace
from steg_tools.affected import Crate, crates_from_metadata


@dataclass(frozen=True)
class BoundaryRule:
    """What one crate may depend on, and why the limit exists."""

    allow: frozenset[str]
    reason: str


@dataclass(frozen=True)
class Violation:
    """A restricted crate's dependency that isn't on its allow list."""

    crate: str
    dependency: str
    reason: str


def read_allow_table(path: Path) -> dict[str, BoundaryRule]:
    """Read boundaries.toml into one `BoundaryRule` per crate it names."""
    with path.open("rb") as handle:
        document = tomllib.load(handle)
    return {
        name: BoundaryRule(frozenset(table["allow"]), table["reason"])
        for name, table in document.items()
    }


def violations(crates: tuple[Crate, ...], rules: dict[str, BoundaryRule]) -> tuple[Violation, ...]:
    """Say which restricted crates depend on something not on their allow list.

    A crate absent from `rules` is unrestricted and never produces a violation.
    """
    found = []
    for crate in crates:
        rule = rules.get(crate.name)
        if rule is None:
            continue
        for dependency in sorted(crate.dependencies - rule.allow):
            found.append(Violation(crate.name, dependency, rule.reason))
    return tuple(found)


def unknown_rules(crates: tuple[Crate, ...], rules: dict[str, BoundaryRule]) -> frozenset[str]:
    """Say which names in `rules` match no real crate - a typo disables the check silently otherwise."""
    names = {crate.name for crate in crates}
    return frozenset(name for name in rules if name not in names)


def format_violation(violation: Violation) -> str:
    """Report one violation, naming the rule it breaks."""
    return (
        f"{violation.crate} depends on {violation.dependency}, which is not on its allow list "
        f"in boundaries.toml ({violation.reason})."
    )


def main() -> int:
    """Check the real workspace's crates against boundaries.toml."""
    root = workspace.repo_root()
    if root is None:
        print("steg-boundaries: not in a git repository", file=sys.stderr)
        return 2
    metadata = workspace.cargo_metadata(root)
    if metadata is None:
        print("steg-boundaries: cargo metadata failed", file=sys.stderr)
        return 1
    crates = crates_from_metadata(metadata, root)
    rules = read_allow_table(root / "boundaries.toml")
    unknown = unknown_rules(crates, rules)
    if unknown:
        for name in sorted(unknown):
            print(f"steg-boundaries: boundaries.toml names {name}, which is not a crate in this workspace.")
        return 1
    found = violations(crates, rules)
    for violation in found:
        print(format_violation(violation))
    return 1 if found else 0
