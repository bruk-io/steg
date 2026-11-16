"""`bare_hsla_returning_functions` against the real `steg-ui` crate's own `.rs` files, not
a fixture nobody has pointed at the actual files."""

from steg_tools import workspace
from steg_tools.foundations_public_surface import (
    REPO_ROOT_RELATIVE_GALLERY_DIR,
    REPO_ROOT_RELATIVE_SRC_DIR,
    bare_hsla_returning_functions,
    rust_files,
)


def test_the_real_crate_has_no_bare_hsla_returning_pub_fn_anywhere() -> None:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    files = rust_files(root, REPO_ROOT_RELATIVE_SRC_DIR, REPO_ROOT_RELATIVE_GALLERY_DIR)
    assert len(files) >= 5, "fewer .rs files than expected - the crate's own layout may have moved"
    found: list[object] = []
    for path in files:
        found.extend(bare_hsla_returning_functions(str(path.relative_to(root)), path.read_text()))
    assert found == []
