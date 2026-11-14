"""The kernel/server boundary: a restricted crate's dependencies against its allow list."""

from pathlib import Path

from steg_tools.affected import Crate
from steg_tools.boundaries import BoundaryRule, Violation, format_violation, read_allow_table, unknown_rules, violations

KERNEL = Crate("steg-kernel", "crates/kernel", frozenset(), frozenset({"serde", "tokio"}))
SERVER = Crate("steg-server", "crates/server", frozenset({"steg-kernel"}), frozenset({"steg-kernel", "tokio"}))
APP = Crate("steg-app", "crates/app", frozenset(), frozenset({"gpui"}))


def test_a_disallowed_dependency_is_a_violation() -> None:
    rules = {"steg-kernel": BoundaryRule(frozenset({"serde"}), "the kernel does no I/O of its own")}
    found = violations((KERNEL,), rules)
    assert found == (Violation("steg-kernel", "tokio", "the kernel does no I/O of its own"),)


def test_every_dependency_allowed_is_no_violation() -> None:
    rules = {"steg-kernel": BoundaryRule(frozenset({"serde", "tokio"}), "the kernel does no I/O of its own")}
    assert violations((KERNEL,), rules) == ()


def test_a_crate_with_no_rule_is_unrestricted() -> None:
    rules = {"steg-kernel": BoundaryRule(frozenset(), "the kernel does no I/O of its own")}
    assert violations((APP,), rules) == ()


def test_more_than_one_restricted_crate_each_report_their_own_violations() -> None:
    rules = {
        "steg-kernel": BoundaryRule(frozenset({"serde"}), "the kernel does no I/O of its own"),
        "steg-server": BoundaryRule(frozenset(), "server deps are reviewed one at a time"),
    }
    found = violations((KERNEL, SERVER), rules)
    assert found == (
        Violation("steg-kernel", "tokio", "the kernel does no I/O of its own"),
        Violation("steg-server", "steg-kernel", "server deps are reviewed one at a time"),
        Violation("steg-server", "tokio", "server deps are reviewed one at a time"),
    )


def test_unknown_rules_names_a_crate_absent_from_the_workspace() -> None:
    rules = {"steg-kernel": BoundaryRule(frozenset(), "x"), "steg-kernal": BoundaryRule(frozenset(), "typo")}
    assert unknown_rules((KERNEL,), rules) == frozenset({"steg-kernal"})


def test_unknown_rules_is_empty_when_every_rule_names_a_real_crate() -> None:
    rules = {"steg-kernel": BoundaryRule(frozenset(), "x")}
    assert unknown_rules((KERNEL, APP), rules) == frozenset()


def test_format_violation_names_the_crate_the_dependency_and_the_reason() -> None:
    message = format_violation(Violation("steg-kernel", "tokio", "the kernel does no I/O of its own"))
    assert "steg-kernel" in message
    assert "tokio" in message
    assert "the kernel does no I/O of its own" in message


def test_read_allow_table_parses_a_real_file(tmp_path: Path) -> None:
    path = tmp_path / "boundaries.toml"
    path.write_text(
        """
        [steg-kernel]
        reason = "the kernel does no I/O of its own"
        allow = ["serde"]
        """
    )
    assert read_allow_table(path) == {
        "steg-kernel": BoundaryRule(frozenset({"serde"}), "the kernel does no I/O of its own"),
    }
