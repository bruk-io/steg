"""`ui_colours.violations` against the real `crates/ui` tree, not a fixture.

True but weak today, for the same reason as test_ui_levels_real_repo.py: foundations is
empty and the gallery is exempt, so nothing under `crates/ui/src` has a real colour
literal yet. This becomes the real regression gate once TASK-1.93 writes
`foundations/generated.rs` (exempt by path) and later tasks add real atoms/molecules/
organisms; the unit suite's fixtures are what exercises the refusal logic today.
"""

from steg_tools import workspace
from steg_tools.foundations import hex_index, read
from steg_tools.rust_source import rust_files
from steg_tools.ui_colours import DESIGN_DATA, GALLERY, UI_SRC, violations


def test_the_real_crate_has_no_colour_violations_today() -> None:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    data_path = root / DESIGN_DATA
    assert data_path.is_file(), "crates/ui/design/foundations.json must exist for this test to mean anything"
    index = hex_index(read(data_path))
    files = rust_files(root, UI_SRC) + rust_files(root, GALLERY)
    assert files, "crates/ui/src and crates/ui/gallery must have files for this test to mean anything"
    assert violations(files, index) == ()
