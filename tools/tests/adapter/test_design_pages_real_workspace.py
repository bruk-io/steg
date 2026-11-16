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
from steg_tools.design_pages import main


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
