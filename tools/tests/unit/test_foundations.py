"""`normalise_hex` and `hex_index` against plain data (tests/adapter/ reads the real file)."""

from pathlib import Path

from steg_tools.foundations import Foundations, NamedColour, hex_index, normalise_hex, read

SURFACE_RAISED = NamedColour("surface.raised", "#1c1b21", "#ffffff")
# outline and track coincide in dark, differ in light - the real coincidence in
# crates/ui/design/foundations.json (its own "use" field says so), reproduced here as a
# synthetic fixture so the unit test doesn't depend on the data file's exact values.
OUTLINE = NamedColour("outline", "#3d3b45", "#bfbbb5")
TRACK = NamedColour("track", "#3d3b45", "#cfcbc5")


def test_normalise_hex_strips_a_hash_prefix_and_lowercases() -> None:
    assert normalise_hex("#1C1B21") == "1c1b21"


def test_normalise_hex_strips_a_0x_prefix_regardless_of_case() -> None:
    assert normalise_hex("0x1c1b21") == "1c1b21"
    assert normalise_hex("0X1C1B21") == "1c1b21"


def test_normalise_hex_leaves_a_bare_value_unchanged_but_lowercased() -> None:
    assert normalise_hex("1C1B21") == "1c1b21"


def test_hex_index_maps_a_hex_value_to_its_one_name_and_form() -> None:
    index = hex_index(Foundations((SURFACE_RAISED,)))
    assert index["1c1b21"] == (("surface.raised", "dark"),)
    assert index["ffffff"] == (("surface.raised", "light"),)


def test_hex_index_lists_every_name_sharing_a_hex_value() -> None:
    index = hex_index(Foundations((OUTLINE, TRACK)))
    assert index["3d3b45"] == (("outline", "dark"), ("track", "dark"))


def test_hex_index_has_no_entry_for_a_hex_value_no_colour_has() -> None:
    index = hex_index(Foundations((SURFACE_RAISED,)))
    assert index.get("abcdef") is None


def test_read_parses_a_minimal_colours_list(tmp_path: Path) -> None:
    path = tmp_path / "foundations.json"
    path.write_text(
        """
        {"colours": [{"name": "surface.raised", "dark": "#1c1b21", "light": "#ffffff", "use": "x"}]}
        """
    )
    assert read(path) == Foundations((SURFACE_RAISED,))
