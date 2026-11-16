"""`bare_hsla_returning_functions` against the real `crates/ui/src/foundations/{mod,
generated}.rs`, not a fixture nobody has pointed at the actual files."""

from steg_tools import workspace
from steg_tools.foundations_public_surface import (
    REPO_ROOT_RELATIVE_GENERATED_FILE,
    REPO_ROOT_RELATIVE_MOD_FILE,
    bare_hsla_returning_functions,
)


def test_the_real_foundations_module_has_no_bare_hsla_returning_pub_fn() -> None:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    found: list[object] = []
    for relative_path in (REPO_ROOT_RELATIVE_MOD_FILE, REPO_ROOT_RELATIVE_GENERATED_FILE):
        found.extend(bare_hsla_returning_functions(relative_path, (root / relative_path).read_text()))
    assert found == []
