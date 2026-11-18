"""What a quiet step prints."""

from pathlib import Path

import pytest

from steg_tools.quiet import (
    effective_slot_count,
    failed_report,
    heavy_cargo_command,
    lock_directory,
    passed_line,
    slug,
    split_arguments,
    tail,
    timed_out_report,
    wait_bound_seconds,
    wait_line,
)


def test_a_pass_is_one_line() -> None:
    assert passed_line("tests", 3.21) == "ok      tests (3.2s)"


def test_a_failure_shows_the_end_of_the_output_and_where_the_rest_is() -> None:
    output = "\n".join(f"line {n}" for n in range(1, 11))
    report = failed_report("lint", 101, 1.0, output, Path(".quiet/lint.log"), limit=3)
    assert report.splitlines() == [
        "FAILED  lint (exit 101, 1.0s)",
        "... 7 earlier lines left out ...",
        "line 8",
        "line 9",
        "line 10",
        "full output: .quiet/lint.log",
    ]


def test_short_output_is_shown_whole() -> None:
    assert tail("a\nb", 60) == "a\nb"


def test_a_timeout_names_the_bound_and_how_long_it_ran() -> None:
    report = timed_out_report("tests (steg-server)", 300.0, 301.2, "a\nb\nc", Path(".quiet/tests.log"), limit=2)
    assert report.splitlines() == [
        "TIMED OUT tests (steg-server) (bound 300s, ran 301.2s)",
        "... 1 earlier lines left out ...",
        "b",
        "c",
        "full output: .quiet/tests.log",
    ]


def test_arguments_need_a_label_a_separator_and_a_command() -> None:
    assert split_arguments(["tests", "--", "cargo", "test"]) == ("tests", ["cargo", "test"])
    with pytest.raises(ValueError, match="usage"):
        split_arguments(["tests", "cargo", "test"])


def test_a_label_becomes_a_file_name() -> None:
    assert slug("tests (-p steg-app)") == "tests-p-steg-app"


def test_cargo_build_test_and_clippy_are_heavy() -> None:
    assert heavy_cargo_command(["cargo", "build", "-p", "steg-sdk"])
    assert heavy_cargo_command(["cargo", "test", "--workspace"])
    assert heavy_cargo_command(["cargo", "clippy", "--workspace", "--all-targets"])


def test_cargo_fmt_and_deny_and_non_cargo_commands_are_not_heavy() -> None:
    assert not heavy_cargo_command(["cargo", "fmt", "--all", "--check"])
    assert not heavy_cargo_command(["cargo", "deny", "check", "licenses"])
    assert not heavy_cargo_command(["uv", "run", "--quiet", "--project", "tools", "steg-boundaries"])
    assert not heavy_cargo_command(["cargo"])
    assert not heavy_cargo_command([])


def test_slot_count_defaults_to_two_outside_ci() -> None:
    assert effective_slot_count({}) == 2


def test_slot_count_honours_the_variable() -> None:
    assert effective_slot_count({"STEG_CHECK_SLOTS": "5"}) == 5


def test_slot_count_zero_means_no_limit() -> None:
    assert effective_slot_count({"STEG_CHECK_SLOTS": "0"}) is None


def test_slot_count_rejects_a_negative_value() -> None:
    with pytest.raises(ValueError, match="must not be negative"):
        effective_slot_count({"STEG_CHECK_SLOTS": "-1"})


def test_slot_count_defaults_to_unlimited_in_ci() -> None:
    assert effective_slot_count({"CI": "true"}) is None


def test_slot_count_in_ci_still_honours_an_explicit_variable() -> None:
    assert effective_slot_count({"CI": "true", "STEG_CHECK_SLOTS": "3"}) == 3


def test_lock_directory_defaults_under_the_home_cache() -> None:
    assert lock_directory({}) == Path.home() / ".cache" / "steg" / "check-slots"


def test_lock_directory_honours_xdg_cache_home() -> None:
    assert lock_directory({"XDG_CACHE_HOME": "/tmp/cache"}) == Path("/tmp/cache/steg/check-slots")


def test_lock_directory_override_wins_over_everything() -> None:
    assert lock_directory({"XDG_CACHE_HOME": "/tmp/cache", "STEG_CHECK_SLOTS_DIR": "/tmp/slots"}) == Path("/tmp/slots")


def test_the_wait_line_names_the_count_and_has_no_bound_by_default() -> None:
    line = wait_line("tests (steg-server)", 2, None)
    assert line.startswith("wait    tests (steg-server) (2 of 2 slots busy, STEG_CHECK_SLOTS=2)")
    assert "STEG_CHECK_SLOT_WAIT_S is unset" in line
    assert "STEG_QUIET_TIMEOUT_S bound has not started yet" in line


def test_the_wait_line_names_the_bound_when_one_is_set() -> None:
    line = wait_line("tests (steg-server)", 2, 120.0)
    assert "told it may wait up to 120s (STEG_CHECK_SLOT_WAIT_S)" in line


def test_wait_bound_is_unset_by_default() -> None:
    assert wait_bound_seconds({}) is None


def test_wait_bound_honours_the_variable() -> None:
    assert wait_bound_seconds({"STEG_CHECK_SLOT_WAIT_S": "45"}) == 45.0
