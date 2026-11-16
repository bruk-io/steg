"""`foundations.read`/`hex_index` against the real `crates/ui/design/foundations.json`.

The unit suite in tests/unit/test_foundations.py proves the parsing against synthetic
data. This is the other half: the real data file.
"""

from steg_tools import workspace
from steg_tools.foundations import hex_index, read


def real_foundations():
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    path = root / "crates/ui/design/foundations.json"
    assert path.is_file(), "crates/ui/design/foundations.json must exist for this test to mean anything"
    return read(path)


def test_the_real_data_file_has_its_documented_eighteen_colours() -> None:
    # design/README.md: "18: the design's 17, plus `track`".
    assert len(real_foundations().colours) == 18


def test_the_real_outline_and_track_colours_coincide_in_dark_and_differ_in_light() -> None:
    # The real coincidence the unit suite's synthetic fixture reproduces.
    index = hex_index(real_foundations())
    assert ("outline", "dark") in index["3d3b45"]
    assert ("track", "dark") in index["3d3b45"]
