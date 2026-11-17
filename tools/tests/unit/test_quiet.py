"""What a quiet step prints."""

from pathlib import Path

import pytest

from steg_tools.quiet import failed_report, passed_line, slug, split_arguments, tail, timed_out_report


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
