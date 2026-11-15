"""Which crates a change affects."""

from pathlib import Path

import pytest

from steg_tools.affected import Crate, crates_from_metadata, owner, selection

CRATES = (
    Crate("steg-kernel", "crates/kernel", frozenset(), frozenset({"serde"})),
    Crate("steg-server", "crates/server", frozenset({"steg-kernel"}), frozenset({"steg-kernel", "tokio"})),
    Crate("steg-app", "crates/app", frozenset({"steg-server"}), frozenset({"steg-server"})),
    Crate("steg-themes", "crates/app/themes", frozenset(), frozenset()),
)


@pytest.mark.parametrize(
    ("changed", "expected"),
    [
        ((), "none"),
        (("README.md", "backlog/tasks/task-1.md", ".claude/rules/licence.md"), "none"),
        (("crates/app/src/main.rs",), "-p steg-app"),
        (("crates/server/src/lib.rs",), "-p steg-app -p steg-server"),
        (("crates/kernel/src/layout.rs",), "-p steg-app -p steg-kernel -p steg-server"),
        (("crates/app/themes/dark.rs",), "-p steg-themes"),
        (("crates/kernel/src/layout.rs", "docs/notes.md"), "-p steg-app -p steg-kernel -p steg-server"),
        (("Cargo.lock",), "all"),
        (("crates/app/src/main.rs", "Cargo.toml"), "all"),
        (("rust-toolchain.toml",), "all"),
        ((".cargo/config.toml",), "all"),
        (("tools/src/steg_tools/quiet.py",), "all"),
        (("justfile",), "all"),
    ],
)
def test_selection(changed: tuple[str, ...], expected: str) -> None:
    assert selection(changed, CRATES) == expected


def test_a_file_belongs_to_the_most_specific_crate() -> None:
    assert owner("crates/app/themes/dark.rs", CRATES) == "steg-themes"
    assert owner("crates/app/src/main.rs", CRATES) == "steg-app"


def test_a_folder_that_only_shares_a_prefix_is_not_the_crate() -> None:
    assert owner("crates/kernel-notes/readme.md", CRATES) is None


def test_crates_are_read_from_cargo_metadata() -> None:
    root = Path("/work/steg")
    metadata = {
        "workspace_members": ["kernel-id", "server-id"],
        "packages": [
            {"id": "kernel-id", "name": "steg-kernel", "manifest_path": "/work/steg/crates/kernel/Cargo.toml", "dependencies": [{"name": "serde"}]},
            {
                "id": "server-id",
                "name": "steg-server",
                "manifest_path": "/work/steg/crates/server/Cargo.toml",
                "dependencies": [{"name": "steg-kernel", "path": "/work/steg/crates/kernel"}, {"name": "tokio"}],
            },
            {"id": "other-id", "name": "not-a-member", "manifest_path": "/work/steg/vendor/x/Cargo.toml", "dependencies": []},
        ],
    }
    # Neither dependency dict says `features`/`uses_default_features`, matching real `cargo
    # metadata` output for an edge with no `features = [...]` and no `default-features = false` -
    # Cargo's own default is "uses default features", so `default_features_on` names both here.
    assert crates_from_metadata(metadata, root) == (
        Crate(
            "steg-kernel",
            "crates/kernel",
            frozenset(),
            frozenset({"serde"}),
            frozenset(),
            frozenset({"serde"}),
        ),
        Crate(
            "steg-server",
            "crates/server",
            frozenset({"steg-kernel"}),
            frozenset({"steg-kernel", "tokio"}),
            frozenset(),
            frozenset({"steg-kernel", "tokio"}),
        ),
    )
