"""The kernel/server boundary: a restricted crate's dependencies against its allow list."""

from pathlib import Path

from steg_tools.affected import Crate
from steg_tools.boundaries import (
    BoundaryRule,
    FeatureViolation,
    Violation,
    feature_violations,
    format_feature_violation,
    format_violation,
    read_allow_table,
    unknown_rules,
    violations,
)

KERNEL = Crate("steg-kernel", "crates/kernel", frozenset(), frozenset({"serde", "tokio"}))
SERVER = Crate("steg-server", "crates/server", frozenset({"steg-kernel"}), frozenset({"steg-kernel", "tokio"}))
APP = Crate("steg-app", "crates/app", frozenset(), frozenset({"gpui"}))

# A kernel that declares steg-sdk with its `local-socket` feature on, as if its own Cargo.toml
# had asked for it - the declared-edge violation this check exists to catch.
KERNEL_WITH_LOCAL_SOCKET = Crate(
    "steg-kernel",
    "crates/kernel",
    frozenset({"steg-sdk"}),
    frozenset({"steg-sdk"}),
    frozenset({("steg-sdk", "local-socket")}),
    frozenset(),
)

# A kernel whose edge to steg-sdk has no named feature but does use its default features - the
# conservative "deny-features on this dependency also forbids its defaults" case.
KERNEL_WITH_DEFAULT_FEATURES = Crate(
    "steg-kernel",
    "crates/kernel",
    frozenset({"steg-sdk"}),
    frozenset({"steg-sdk"}),
    frozenset(),
    frozenset({"steg-sdk"}),
)


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


def test_read_allow_table_parses_a_deny_features_table(tmp_path: Path) -> None:
    path = tmp_path / "boundaries.toml"
    path.write_text(
        """
        [steg-kernel]
        reason = "the kernel does no I/O of its own, never through the sdk's optional parts"
        allow = ["steg-sdk"]
        deny-features = { "steg-sdk" = ["local-socket"] }
        """
    )
    assert read_allow_table(path) == {
        "steg-kernel": BoundaryRule(
            frozenset({"steg-sdk"}),
            "the kernel does no I/O of its own, never through the sdk's optional parts",
            frozenset({("steg-sdk", "local-socket")}),
        ),
    }


def test_a_denied_feature_declared_on_the_edge_is_a_violation() -> None:
    rules = {
        "steg-kernel": BoundaryRule(
            frozenset({"steg-sdk"}), "no optional parts", frozenset({("steg-sdk", "local-socket")})
        )
    }
    found = feature_violations((KERNEL_WITH_LOCAL_SOCKET,), rules)
    assert found == (FeatureViolation("steg-kernel", "steg-sdk", "local-socket", "no optional parts"),)


def test_an_allowed_dependency_with_no_denied_feature_used_is_no_violation() -> None:
    rules = {
        "steg-kernel": BoundaryRule(frozenset({"steg-sdk"}), "no optional parts", frozenset({("steg-sdk", "local-socket")}))
    }
    assert feature_violations((KERNEL,), rules) == ()


def test_a_crate_whose_rule_denies_no_feature_has_no_feature_violations() -> None:
    rules = {"steg-kernel": BoundaryRule(frozenset({"steg-sdk"}), "no optional parts")}
    assert feature_violations((KERNEL_WITH_DEFAULT_FEATURES,), rules) == ()


def test_default_features_on_a_denied_dependency_is_a_violation() -> None:
    rules = {
        "steg-kernel": BoundaryRule(
            frozenset({"steg-sdk"}), "no optional parts", frozenset({("steg-sdk", "local-socket")})
        )
    }
    found = feature_violations((KERNEL_WITH_DEFAULT_FEATURES,), rules)
    assert found == (FeatureViolation("steg-kernel", "steg-sdk", "default", "no optional parts"),)


def test_format_feature_violation_names_the_crate_the_dependency_the_feature_and_the_reason() -> None:
    message = format_feature_violation(FeatureViolation("steg-kernel", "steg-sdk", "local-socket", "no optional parts"))
    assert "steg-kernel" in message
    assert "steg-sdk" in message
    assert "local-socket" in message
    assert "no optional parts" in message
