"""`ui_colours.violations` against the real `crates/ui` tree, not a fixture.

The real regression gate, not a weak placeholder: TASK-1.93 wrote `foundations/
generated.rs` (exempt by path, R121) and made the gallery draw every colour it shows
from `foundations` by name, so the gallery's own, now-ended exemption (`ui_colours.py`'s
`EXEMPT_PREFIXES`) has nothing left to cover for it - this asserts that directly against
the real tree. Still weak for atoms/molecules/organisms specifically, which stay empty
placeholders until later tasks add real components.
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
