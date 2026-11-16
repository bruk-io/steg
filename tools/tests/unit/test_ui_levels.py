"""`ui_levels`'s pure functions against plain data and source fixtures (tests/adapter/ reads the real crate)."""

from ast_grep_py import SgRoot

from steg_tools.rust_source import RustFile
from steg_tools.ui_levels import (
    LEVELS,
    Violation,
    crate_root_exports,
    first_level_segment,
    format_unplaced,
    format_violation,
    level_of_file,
    resolve_path,
    unplaced_files,
    use_paths,
    violations,
)


def use_declaration(source: str):
    """Parse one `use` statement and return its `use_declaration` node, for `use_paths`."""
    return SgRoot(source, "rust").root().find(kind="use_declaration")


# --- level_of_file ----------------------------------------------------------------


def test_level_of_file_names_each_of_the_four_levels() -> None:
    for level in LEVELS:
        assert level_of_file(f"crates/ui/src/{level}/mod.rs") == level


def test_level_of_file_is_none_for_lib_rs() -> None:
    assert level_of_file("crates/ui/src/lib.rs") is None


def test_level_of_file_is_none_outside_crates_ui_src() -> None:
    assert level_of_file("crates/ui/gallery/main.rs") is None
    assert level_of_file("crates/app/src/main.rs") is None


def test_level_of_file_is_none_for_a_file_placed_directly_under_src() -> None:
    assert level_of_file("crates/ui/src/util.rs") is None


# --- unplaced_files -----------------------------------------------------------------


def test_unplaced_files_names_a_file_directly_under_src() -> None:
    files = (
        RustFile("crates/ui/src/util.rs", ""),
        RustFile("crates/ui/src/lib.rs", ""),
        RustFile("crates/ui/src/atoms/mod.rs", ""),
    )
    assert unplaced_files(files) == ("crates/ui/src/util.rs",)


def test_unplaced_files_is_empty_when_every_file_is_placed_or_lib_rs() -> None:
    files = (RustFile("crates/ui/src/lib.rs", ""), RustFile("crates/ui/src/atoms/mod.rs", ""))
    assert unplaced_files(files) == ()


# --- first_level_segment ------------------------------------------------------------


def test_first_level_segment_recognises_a_level_name() -> None:
    assert first_level_segment("crate::atoms::Button") == "atoms"


def test_first_level_segment_is_none_for_a_name_that_is_not_a_level() -> None:
    assert first_level_segment("crate::Button") is None
    assert first_level_segment("gpui::Hsla") is None


# --- crate_root_exports --------------------------------------------------------------


def test_crate_root_exports_resolves_a_plain_pub_use() -> None:
    assert crate_root_exports("pub use crate::atoms::Button;\n") == {"Button": "atoms"}


def test_crate_root_exports_resolves_an_aliased_pub_use() -> None:
    assert crate_root_exports("pub use crate::atoms::Button as Btn;\n") == {"Btn": "atoms"}


def test_crate_root_exports_does_not_expand_a_glob_pub_use() -> None:
    # The deliberate, named gap: proven inert, not merely described.
    assert crate_root_exports("pub use crate::atoms::*;\n") == {}


def test_crate_root_exports_ignores_a_use_with_no_pub() -> None:
    assert crate_root_exports("use crate::atoms::Button;\n") == {}


def test_crate_root_exports_ignores_a_pub_use_of_something_outside_the_four_levels() -> None:
    assert crate_root_exports("pub use anyhow::Result;\n") == {}


def test_crate_root_exports_today_is_empty_for_four_bare_pub_mod_declarations() -> None:
    # What crates/ui/src/lib.rs holds today (TASK-1.91) - no pub use at all.
    assert crate_root_exports("pub mod atoms;\npub mod foundations;\n") == {}


# --- resolve_path ---------------------------------------------------------------------


def test_resolve_path_resolves_a_crate_prefixed_level_path() -> None:
    assert resolve_path("crate::atoms::Button", {}) == "atoms"


def test_resolve_path_resolves_through_a_crate_root_reexport() -> None:
    assert resolve_path("crate::Button", {"Button": "atoms"}) == "atoms"


def test_resolve_path_is_none_for_a_crate_name_with_no_matching_export() -> None:
    # The same case a glob re-export leaves unresolved (crate_root_exports' own gap).
    assert resolve_path("crate::Button", {}) is None


def test_resolve_path_resolves_a_single_super_hop_to_a_sibling_level() -> None:
    assert resolve_path("super::organisms::Bar", {}) == "organisms"


def test_resolve_path_is_none_for_a_second_super_hop() -> None:
    # The named limitation: proven inert (None, not a crash or a guess), not merely described.
    assert resolve_path("super::super::organisms::Bar", {}) is None


def test_resolve_path_is_none_for_an_external_crate_path() -> None:
    assert resolve_path("gpui::Hsla", {}) is None


def test_resolve_path_is_none_for_self_path() -> None:
    assert resolve_path("self::Thing", {}) is None


# --- use_paths (flattening) ------------------------------------------------------------


def test_use_paths_reads_a_plain_use() -> None:
    assert use_paths(use_declaration("use crate::atoms::Icon;")) == ("crate::atoms::Icon",)


def test_use_paths_flattens_a_grouped_use() -> None:
    declaration = use_declaration("use crate::{atoms::Button, molecules::Tab};")
    assert use_paths(declaration) == ("crate::atoms::Button", "crate::molecules::Tab")


def test_use_paths_resolves_a_glob_to_its_module_path() -> None:
    assert use_paths(use_declaration("use crate::atoms::*;")) == ("crate::atoms",)


def test_use_paths_drops_an_alias_but_keeps_the_path() -> None:
    assert use_paths(use_declaration("use crate::atoms::Button as Btn;")) == ("crate::atoms::Button",)


# --- violations: the main mechanism, one test per adjacent and non-adjacent crossing -----


def test_violations_refuses_foundations_importing_atoms() -> None:
    file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::atoms::Icon;\n")
    found = violations((file,), {})
    assert found == (Violation(file.path, 1, "use crate::atoms::Icon;", "foundations", "atoms"),)


def test_violations_refuses_atoms_importing_molecules() -> None:
    file = RustFile("crates/ui/src/atoms/mod.rs", "use crate::molecules::Tab;\n")
    found = violations((file,), {})
    assert found == (Violation(file.path, 1, "use crate::molecules::Tab;", "atoms", "molecules"),)


def test_violations_refuses_molecules_importing_organisms() -> None:
    file = RustFile("crates/ui/src/molecules/mod.rs", "use crate::organisms::Bar;\n")
    found = violations((file,), {})
    assert found == (Violation(file.path, 1, "use crate::organisms::Bar;", "molecules", "organisms"),)


def test_violations_refuses_foundations_importing_organisms_directly() -> None:
    # Non-adjacent: a comparison written as "exactly one level up" would miss this.
    file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::organisms::Bar;\n")
    found = violations((file,), {})
    assert found == (Violation(file.path, 1, "use crate::organisms::Bar;", "foundations", "organisms"),)


def test_violations_passes_an_organism_using_a_molecule_an_atom_and_foundations() -> None:
    file = RustFile(
        "crates/ui/src/organisms/mod.rs",
        "use crate::molecules::Tab;\nuse crate::atoms::Icon;\nuse crate::foundations::Colour;\n",
    )
    assert violations((file,), {}) == ()


def test_violations_passes_an_organism_using_foundations_directly() -> None:
    # Two levels down: a comparison written as "exactly one level" would refuse this.
    file = RustFile("crates/ui/src/organisms/mod.rs", "use crate::foundations::Colour;\n")
    assert violations((file,), {}) == ()


def test_violations_refuses_an_import_reached_only_through_a_crate_root_reexport() -> None:
    file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::Button;\n")
    found = violations((file,), {"Button": "atoms"})
    assert found == (Violation(file.path, 1, "use crate::Button;", "foundations", "atoms"),)


def test_crate_root_reexport_loophole_is_closed_end_to_end() -> None:
    # Not a hand-built exports dict: the real lib.rs text, through the real builder,
    # fed to the real violations() - the whole path the loophole-closing claim rests on.
    lib_rs_text = "pub mod atoms;\npub mod foundations;\npub use atoms::Button;\n"
    exports = crate_root_exports(lib_rs_text)
    file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::Button;\n")
    found = violations((file,), exports)
    assert found == (Violation(file.path, 1, "use crate::Button;", "foundations", "atoms"),)


def test_violations_does_not_flag_a_reexport_this_check_cannot_resolve() -> None:
    # The same import as above, but with no export table entry - the known, proven-inert gap.
    file = RustFile("crates/ui/src/foundations/mod.rs", "use crate::Button;\n")
    assert violations((file,), {}) == ()


def test_violations_counts_a_fully_qualified_expression_path_once_not_once_per_nesting() -> None:
    file = RustFile("crates/ui/src/foundations/mod.rs", "fn f() { let _ = crate::atoms::Button::new(); }\n")
    found = violations((file,), {})
    assert len(found) == 1
    assert found[0].reached_level == "atoms"


def test_violations_does_not_see_a_crossing_path_written_inside_a_macros_arguments() -> None:
    # The named, documented blind spot (this module's own docstring): tree-sitter hands
    # a macro's parenthesised body back as one opaque token tree, never a scoped_identifier.
    file = RustFile("crates/ui/src/foundations/mod.rs", "fn f() { some_macro!(crate::atoms::Button); }\n")
    assert violations((file,), {}) == ()


def test_violations_ignores_an_unplaced_file() -> None:
    # level_of_file returns None for it, so this function has nothing to check it against -
    # unplaced_files (above) is what refuses it, loudly, not this function silently.
    file = RustFile("crates/ui/src/util.rs", "use crate::organisms::Bar;\n")
    assert violations((file,), {}) == ()


# --- formatting --------------------------------------------------------------------------


def test_format_violation_names_both_levels_the_import_and_the_rule() -> None:
    message = format_violation(Violation("crates/ui/src/foundations/mod.rs", 3, "use crate::atoms::Icon;", "foundations", "atoms"))
    assert "foundations" in message
    assert "atoms" in message
    assert "R119" in message
    assert "use crate::atoms::Icon;" in message


def test_format_unplaced_names_the_four_levels() -> None:
    message = format_unplaced("crates/ui/src/util.rs")
    assert "crates/ui/src/util.rs" in message
    for level in LEVELS:
        assert level in message
