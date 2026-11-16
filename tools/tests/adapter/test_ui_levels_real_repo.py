"""`ui_levels.violations` against the real `crates/ui/src` tree, not a fixture.

True but weak today: every real file is an empty, doc-comment-only module (TASK-1.91),
so this test can't yet distinguish "the check works" from "there is nothing to check".
It becomes the real regression gate once TASK-1.93 and later tasks add real components;
the unit suite's fixtures (tests/unit/test_ui_levels.py) are what actually exercises the
refusal and pass logic today.
"""

from steg_tools import workspace
from steg_tools.rust_source import rust_files
from steg_tools.ui_levels import UI_LIB_RS, UI_SRC, crate_root_exports, unplaced_files, violations


def test_the_real_crate_has_no_level_violations_today() -> None:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    files = rust_files(root, UI_SRC)
    assert files, "crates/ui/src must have files for this test to mean anything"
    lib_rs = root / UI_LIB_RS
    exports = crate_root_exports(lib_rs.read_text()) if lib_rs.is_file() else {}
    assert violations(files, exports) == ()
    assert unplaced_files(files) == ()


def test_the_real_lib_rs_re_exports_nothing_today() -> None:
    # Documents the fact crate_root_exports' glob/grouped gaps currently cost nothing:
    # lib.rs only `pub mod`s the four levels (TASK-1.91), no `pub use` of any shape.
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    lib_rs = root / UI_LIB_RS
    assert lib_rs.is_file(), "crates/ui/src/lib.rs must exist for this test to mean anything"
    assert crate_root_exports(lib_rs.read_text()) == {}
