"""`rust_files` against a scratch tree, not the real repository (tests/adapter/ does that)."""

from pathlib import Path

from steg_tools.rust_source import RustFile, rust_files


def test_rust_files_finds_every_rs_file_recursively(tmp_path: Path) -> None:
    (tmp_path / "crates" / "ui" / "src" / "atoms").mkdir(parents=True)
    (tmp_path / "crates" / "ui" / "src" / "lib.rs").write_text("pub mod atoms;\n")
    (tmp_path / "crates" / "ui" / "src" / "atoms" / "mod.rs").write_text("//! atoms\n")
    (tmp_path / "crates" / "ui" / "src" / "atoms" / "README.md").write_text("not rust\n")

    files = rust_files(tmp_path, "crates/ui/src")

    assert files == (
        RustFile("crates/ui/src/atoms/mod.rs", "//! atoms\n"),
        RustFile("crates/ui/src/lib.rs", "pub mod atoms;\n"),
    )


def test_rust_files_is_empty_for_a_folder_that_does_not_exist(tmp_path: Path) -> None:
    assert rust_files(tmp_path, "crates/ui/src") == ()
