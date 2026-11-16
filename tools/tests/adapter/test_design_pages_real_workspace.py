"""`steg-design-check`'s CLI behaviour against the real repository: the one real boundary it has.

Reads the real `design/canvas/project/{Tokens,Details}.dc.html` always; `--data-file`
points the data-file half at a scratch copy for the negative controls, so the repository's
real pages stay untouched (per `team:testing`: real files, nothing mocked, only the input
this check exists to validate is swapped for a controlled one).
"""

import json
import shutil
from pathlib import Path

from steg_tools import workspace
from steg_tools.design_pages import TIER_C_SKIPS, main, unaccounted_names


def real_data_path() -> Path:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    return root / "crates/ui/design/foundations.json"


def test_the_real_data_file_passes_against_the_real_saved_pages_today() -> None:
    assert main([]) == 0


def test_a_changed_dark_hex_in_a_scratch_copy_is_refused_naming_both_forms(tmp_path: Path, capsys: object) -> None:
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    for entry in data["colours"]:
        if entry["name"] == "surface.ground":
            assert entry["dark"] == "#0e0d10"
            entry["dark"] = "#123456"
    scratch.write_text(json.dumps(data))

    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "surface.ground" in captured.out
    assert "#123456" in captured.out
    assert "#0e0d10" in captured.out


def test_a_removed_colour_entry_in_a_scratch_copy_is_reported_missing(tmp_path: Path, capsys: object) -> None:
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    before = len(data["colours"])
    data["colours"] = [entry for entry in data["colours"] if entry["name"] != "surface.ground"]
    assert len(data["colours"]) == before - 1
    scratch.write_text(json.dumps(data))

    # The real pages still draw surface.ground - reported by the reverse direction, not
    # by Tier A (which only ever looks at names the file still has).
    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "surface.ground" in captured.out
    assert "no entry" in captured.out


def test_copying_the_real_file_byte_for_byte_still_passes(tmp_path: Path) -> None:
    # Sanity check for the two negative controls above: the harness itself (a bare
    # scratch copy, no mutation) must still pass, so a failure above is the mutation's
    # doing, not an artefact of reading from tmp_path.
    scratch = tmp_path / "foundations.json"
    shutil.copyfile(real_data_path(), scratch)
    assert main(["--data-file", str(scratch)]) == 0


def test_a_changed_corner_in_a_scratch_copy_is_refused(tmp_path: Path, capsys: object) -> None:
    # The engineer's small, reviewed map (RADIUS_MAP): sizes.radius.surface/track/inner
    # (14/10/8) each check their own number against Tokens.dc.html's one merged "radius"
    # row - this is the negative control proving the real data file's three mapped
    # entries do pass against the real page today (test_the_real_data_file_passes_
    # against_the_real_saved_pages_today above already proves the positive case).
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    assert data["sizes"]["radius.surface"]["value"] == 14
    data["sizes"]["radius.surface"]["value"] = 99
    scratch.write_text(json.dumps(data))

    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "radius.surface" in captured.out
    assert "99" in captured.out


def test_the_fully_round_corner_is_listed_as_unseen_in_the_tools_own_output(capsys: object) -> None:
    assert main([]) == 0
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "sizes.radius.round" in captured.out
    assert "sizes.radius.surface" not in captured.out  # no longer a declared exception


def test_swapping_tab_widths_usual_and_min_in_a_scratch_copy_is_refused(tmp_path: Path, capsys: object) -> None:
    # The reviewer's own negative control (TASK-1.96 review finding): the same two
    # numbers stay present on the page, swapped between two fields - a plain "every
    # number is present somewhere" check would miss this; FIELD_WORD_MAPS must not.
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    tab_width = data["sizes"]["tab.width"]
    assert (tab_width["usual"], tab_width["min"]) == (120, 72)
    tab_width["usual"], tab_width["min"] = tab_width["min"], tab_width["usual"]
    scratch.write_text(json.dumps(data))

    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "tab.width.usual" in captured.out
    assert "tab.width.min" in captured.out


def test_the_tools_passing_output_names_the_field_blind_caveat(capsys: object) -> None:
    assert main([]) == 0
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "field-blind" in captured.out


def test_every_leaf_key_of_the_real_data_file_is_checked_or_declared_unseen() -> None:
    data = json.loads(real_data_path().read_text())
    assert unaccounted_names(data) == ()


def test_removing_a_declared_unseen_entry_makes_that_key_unaccounted(tmp_path: Path) -> None:
    # Required negative control: a declared-unseen entry removed, the key it covered
    # must show up as unaccounted, not fall through silently.
    data = json.loads(real_data_path().read_text())
    skipped_without_one = frozenset(skip.name for skip in TIER_C_SKIPS) - {"session_colour"}
    found = unaccounted_names(data, skipped=skipped_without_one)
    assert found == ("session_colour",)
