"""`crates_from_metadata` against this repository's real `cargo metadata`, not a made-up fixture.

The unit suite in tests/unit/test_affected.py proves `crates_from_metadata`'s parsing against
JSON nobody has ever run `cargo metadata` to produce. This is the other half: the real thing.
"""

from steg_tools import workspace
from steg_tools.affected import Crate, crates_from_metadata, selection


def real_crates() -> tuple[Crate, ...]:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    metadata = workspace.cargo_metadata(root)
    assert metadata is not None, "cargo metadata must succeed for this test to mean anything"
    return crates_from_metadata(metadata, root)


def test_the_five_real_crates_and_their_real_dependency_edges() -> None:
    by_name = {crate.name: crate for crate in real_crates()}
    assert by_name.keys() == {"steg-kernel", "steg-server", "steg-cli", "steg", "steg-app"}

    assert by_name["steg-kernel"].folder == "crates/kernel"
    assert by_name["steg-kernel"].depends_on == frozenset()

    assert by_name["steg-server"].folder == "crates/server"
    assert by_name["steg-server"].depends_on == frozenset({"steg-kernel"})
    assert "steg-kernel" in by_name["steg-server"].dependencies

    assert by_name["steg-cli"].folder == "crates/cli"
    assert by_name["steg-cli"].depends_on == frozenset()

    assert by_name["steg"].folder == "crates/steg"
    assert by_name["steg"].depends_on == frozenset({"steg-server", "steg-cli"})

    assert by_name["steg-app"].folder == "crates/app"
    assert by_name["steg-app"].depends_on == frozenset()
    assert "gpui" in by_name["steg-app"].dependencies  # external, not a workspace path dependency


def test_selection_against_the_real_crates() -> None:
    crates = real_crates()
    assert selection(("crates/kernel/src/lib.rs",), crates) == "-p steg -p steg-kernel -p steg-server"
    assert selection(("crates/app/src/main.rs",), crates) == "-p steg-app"
    assert selection(("crates/cli/src/lib.rs",), crates) == "-p steg -p steg-cli"
