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
    """What one crate may depend on, which of an allowed dependency's features it may not turn on, and why."""

    allow: frozenset[str]
    reason: str
    deny_features: frozenset[tuple[str, str]] = frozenset()


@dataclass(frozen=True)
class Violation:
    """A restricted crate's dependency that isn't on its allow list."""

    crate: str
    dependency: str
    reason: str


@dataclass(frozen=True)
class FeatureViolation:
    """A restricted crate's dependency edge that turns on a feature its rule denies.

    `feature` is `"default"` when the edge has `uses_default_features: true` on a dependency that
    has any feature denied at all - deliberately conservative: the sdk has no `default` feature
    today, so this never fires, but it refuses one being added later without this check noticing.
    """

    crate: str
    dependency: str
    feature: str
    reason: str


def read_allow_table(path: Path) -> dict[str, BoundaryRule]:
    """Read boundaries.toml into one `BoundaryRule` per crate it names."""
    with path.open("rb") as handle:
        document = tomllib.load(handle)
    return {
        name: BoundaryRule(
            frozenset(table["allow"]),
            table["reason"],
            frozenset(
                (dependency, feature)
                for dependency, features in table.get("deny-features", {}).items()
                for feature in features
            ),
        )
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


def feature_violations(crates: tuple[Crate, ...], rules: dict[str, BoundaryRule]) -> tuple[FeatureViolation, ...]:
    """Say which restricted crates turn on a feature their rule denies, by declaration or by default.

    Reads each crate's *declared* dependency edges (`Crate.dependency_features`,
    `Crate.default_features_on`), not Cargo's resolved, workspace-unified feature set - the point of
    this check is to catch a crate's own `Cargo.toml` asking for a denied feature, which stays
    visible even when, elsewhere in the same `--workspace` build, some other crate's edge has
    already turned that feature on for everyone (see `boundaries.toml`'s own comment).
    """
    found = []
    for crate in crates:
        rule = rules.get(crate.name)
        if rule is None or not rule.deny_features:
            continue
        for dependency, feature in sorted(rule.deny_features):
            if (dependency, feature) in crate.dependency_features:
                found.append(FeatureViolation(crate.name, dependency, feature, rule.reason))
        denied_dependencies = {dependency for dependency, _ in rule.deny_features}
        for dependency in sorted(denied_dependencies & crate.default_features_on):
            found.append(FeatureViolation(crate.name, dependency, "default", rule.reason))
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


def format_feature_violation(violation: FeatureViolation) -> str:
    """Report one feature violation, naming the crate, the dependency, the feature and the reason."""
    return (
        f'{violation.crate} turns on {violation.dependency}\'s "{violation.feature}" feature, '
        f"which its rule in boundaries.toml denies ({violation.reason})."
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
    found_features = feature_violations(crates, rules)
    for violation in found_features:
        print(format_feature_violation(violation))
    return 1 if found or found_features else 0
