"""`normalise_hex` and `hex_index` against plain data (tests/adapter/ reads the real file)."""

from pathlib import Path

from steg_tools.foundations import Foundations, NamedColour, hex_index, normalise_hex, opacity_names_from_document, read

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


# --- opacity_names_from_document: each of the four shapes, and none ----------------------


def test_opacity_names_from_document_reads_the_opacities_list() -> None:
    document = {"opacities": [{"name": "faint", "dark": 0.3, "light": 0.45}]}
    assert opacity_names_from_document(document) == ("faint",)


def test_opacity_names_from_document_reads_tint_because_it_carries_its_own_opacity() -> None:
    document = {"tint": {"name": "tint", "colour": "session", "opacity": 0.18}}
    assert opacity_names_from_document(document) == ("tint",)


def test_opacity_names_from_document_ignores_a_tint_like_value_with_no_opacity_field() -> None:
    document = {"tint": {"name": "tint", "colour": "session"}}
    assert opacity_names_from_document(document) == ()


def test_opacity_names_from_document_ignores_a_ramp_with_no_caption_opacity() -> None:
    # ramp.extended has stops but no caption - contributes nothing.
    document = {"ramps": [{"name": "ramp.extended", "stops": []}]}
    assert opacity_names_from_document(document) == ()


def test_opacity_names_from_document_reads_a_ramp_that_has_caption_opacity() -> None:
    document = {"ramps": [{"name": "ramp", "stops": [], "caption_opacity": {"dark": 0.5, "light": 0.4}}]}
    assert opacity_names_from_document(document) == ("ramp",)


def test_opacity_names_from_document_reads_the_scrim_key_under_sizes() -> None:
    # scrim has no "name" field of its own - it's a key in sizes, read by that key.
    document = {"sizes": {"scrim": {"dark": {"colour": "#000000", "opacity": 0.6}}}}
    assert opacity_names_from_document(document) == ("scrim",)


def test_opacity_names_from_document_is_empty_for_a_document_with_none_of_the_four_shapes() -> None:
    assert opacity_names_from_document({}) == ()


def test_opacity_names_from_document_combines_all_four_and_removing_one_removes_its_name() -> None:
    document = {
        "opacities": [{"name": "faint"}],
        "tint": {"name": "tint", "opacity": 0.18},
        "ramps": [{"name": "ramp", "caption_opacity": {}}],
        "sizes": {"scrim": {}},
    }
    assert opacity_names_from_document(document) == ("faint", "tint", "ramp", "scrim")

    without_tint = {key: value for key, value in document.items() if key != "tint"}
    assert opacity_names_from_document(without_tint) == ("faint", "ramp", "scrim")
