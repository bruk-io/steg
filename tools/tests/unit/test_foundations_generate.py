"""The generator's pure core (TASK-1.93's plan): identifiers, the three-way colour
discrimination, the two-hop `tint` -> `"session"` chain, the alias double-wrap, and
scrim's directional light->text resolution - proven on the renderer's own output, since
that's what actually reaches `generated.rs`, not a resolver the renderer doesn't call.
No I/O anywhere in this file - the one real-file boundary is
tests/adapter/test_foundations_generate_real_data.py.
"""

import pytest

from steg_tools.foundations_generate import (
    Appearance,
    IdentifierCollisionError,
    Ref,
    Session,
    assign_identifiers,
    bake_colour_for_appearance,
    classify,
    identifier,
    is_stale,
    pascal_case,
    render,
    render_colours,
    render_focus_ring,
    render_plain_group,
    render_ramps,
    render_scrim,
    render_sizes,
    render_tint,
    resolve_size_ref,
    staleness_message,
)
from steg_tools.foundations_generate import design_entries as entries_of


# ---------------------------------------------------------------------------
# identifiers
# ---------------------------------------------------------------------------


def test_a_dotted_name_becomes_snake_case() -> None:
    assert identifier("surface.ground") == "surface_ground"


def test_a_spaced_and_numbered_name_becomes_snake_case() -> None:
    assert identifier("ramp.1 violet") == "ramp_1_violet"


def test_a_hyphenated_name_becomes_snake_case() -> None:
    assert identifier("motion.status-fade") == "motion_status_fade"


def test_pascal_case_from_an_identifier() -> None:
    assert pascal_case("nav_side") == "NavSide"
    assert pascal_case("font_ui") == "FontUi"


def test_assign_identifiers_maps_every_distinct_name() -> None:
    assert assign_identifiers(("surface.ground", "text")) == {"surface.ground": "surface_ground", "text": "text"}


def test_assign_identifiers_raises_on_a_collision_naming_both_names() -> None:
    with pytest.raises(IdentifierCollisionError) as excinfo:
        assign_identifiers(("ramp 1", "ramp.1"))
    message = str(excinfo.value)
    assert "ramp 1" in message
    assert "ramp.1" in message


# ---------------------------------------------------------------------------
# the three-way colour discrimination (design/README.md's own rule)
# ---------------------------------------------------------------------------


def test_a_ref_object_classifies_as_a_ref() -> None:
    assert classify({"ref": "surface.ground"}) == Ref("surface.ground")


def test_a_bare_hex_string_classifies_as_itself() -> None:
    assert classify("#0e0d10") == "#0e0d10"


def test_the_bare_string_session_classifies_as_session() -> None:
    assert classify("session") == Session()


def test_anything_else_is_refused() -> None:
    with pytest.raises(ValueError, match="not a colour value"):
        classify(42)


# ---------------------------------------------------------------------------
# resolve_size_ref: track.layout's radius_outer/radius_inner-style plain refs
# ---------------------------------------------------------------------------


def test_a_plain_number_resolves_to_itself() -> None:
    assert resolve_size_ref(10, {}) == 10


def test_a_ref_to_a_sized_entry_resolves_to_its_value_field() -> None:
    index = {"radius.track": {"name": "radius.track", "value": 10, "unit": "px"}}
    assert resolve_size_ref({"ref": "radius.track"}, index) == 10


def test_a_ref_to_something_with_no_value_field_is_refused() -> None:
    index = {"tint": {"name": "tint", "colour": "session", "opacity": 0.18}}
    with pytest.raises(ValueError, match="no plain numeric value"):
        resolve_size_ref({"ref": "tint"}, index)


# ---------------------------------------------------------------------------
# bake_colour_for_appearance: the terminal themes' only resolution need (no `self`
# to delegate through, since they're free functions, not `Theme` methods)
# ---------------------------------------------------------------------------


def test_a_literal_bakes_to_itself_regardless_of_appearance() -> None:
    assert bake_colour_for_appearance("#1c1b21", Appearance.DARK, {}) == "#1c1b21"
    assert bake_colour_for_appearance("#1c1b21", Appearance.LIGHT, {}) == "#1c1b21"


def test_a_ref_bakes_to_the_named_colours_form_for_that_appearance() -> None:
    index = {"surface.pane": {"name": "surface.pane", "dark": "#141318", "light": "#f8f6f4"}}
    assert bake_colour_for_appearance({"ref": "surface.pane"}, Appearance.DARK, index) == "#141318"
    assert bake_colour_for_appearance({"ref": "surface.pane"}, Appearance.LIGHT, index) == "#f8f6f4"


def test_session_is_refused_rather_than_silently_resolved() -> None:
    with pytest.raises(ValueError, match="session"):
        bake_colour_for_appearance("session", Appearance.DARK, {})


# ---------------------------------------------------------------------------
# design_entries: the generic "one name per addressable entry" walk AC#4 relies on
# ---------------------------------------------------------------------------


def test_a_list_section_yields_one_entry_per_items_own_name() -> None:
    data = {"colours": [{"name": "text", "dark": "#000", "light": "#fff"}]}
    assert entries_of(data) == (("colours", "text", data["colours"][0]),)


def test_a_dict_of_groups_section_yields_one_entry_per_key() -> None:
    data = {"sizes": {"space": {"steps": [2, 4], "unit": "px"}}}
    assert entries_of(data) == (("sizes", "space", data["sizes"]["space"]),)


def test_a_lone_object_with_its_own_name_field_yields_that_name() -> None:
    data = {"tint": {"name": "tint", "colour": "session", "opacity": 0.18}}
    assert entries_of(data) == (("tint", "tint", data["tint"]),)


def test_a_lone_object_with_no_name_field_yields_its_own_section_key() -> None:
    data = {"focus_ring": {"width": 2, "unit": "px", "offset": 2, "colour": "session"}}
    assert entries_of(data) == (("focus_ring", "focus_ring", data["focus_ring"]),)


def test_the_three_metadata_keys_are_never_entries() -> None:
    data = {"artifact_version": "x", "artifact_url": "y", "session_colour": "session"}
    assert entries_of(data) == ()


def test_an_unrecognised_top_level_shape_is_refused() -> None:
    with pytest.raises(ValueError, match="unexpected top-level shape"):
        entries_of({"weird": "a bare string, not a list or dict"})


# ---------------------------------------------------------------------------
# the renderers that carry a reference through: aliases, the two-hop
# track.layout -> tint -> session chain, and scrim's directional resolution
# ---------------------------------------------------------------------------


def test_a_colour_alias_delegates_to_the_colour_it_names_rather_than_duplicating_its_hex() -> None:
    data = {
        "colours": [{"name": "ramp.5 amber", "dark": "#ffc25e", "light": "#8f5300", "use": "x"}],
        "colour_aliases": [{"name": "blocked", "ref": {"ref": "ramp.5 amber"}, "use": "Waiting status"}],
    }
    _, methods = render_colours(data)
    alias_method = next(method for method in methods if "pub fn blocked" in method)
    assert "self.ramp_5_amber()" in alias_method
    assert "#ffc25e" not in alias_method  # delegates, never bakes its own copy of the hex


def test_tint_resolves_session_directly_since_its_the_one_place_session_enters() -> None:
    data = {"tint": {"name": "tint", "colour": "session", "opacity": 0.18, "use": "x"}}
    (method,) = render_tint(data)
    assert "self.session" in method


def test_track_layout_fill_is_the_second_hop_of_the_tint_to_session_chain() -> None:
    # track.layout.fill.colour: {"ref": "tint"} - one hop at render time (delegates to
    # self.tint()), whose own body (proven above) is the second hop, to self.session.
    sizes = {"track.layout": {"unit": "px", "inset": 2, "gap": 2, "fill": {"colour": {"ref": "tint"}}, "use": "x"}}
    _, methods = render_sizes({"sizes": sizes})
    (fill_method,) = methods
    assert "track_layout_fill" in fill_method
    assert "self.tint()" in fill_method


def test_scrims_light_branch_delegates_to_texts_own_method_not_a_baked_literal() -> None:
    scrim = {
        "dark": {"colour": "#050508", "opacity": 0.60},
        "light": {"colour": {"ref": "text"}, "opacity": 0.28},
        "use": "x",
    }
    method = render_scrim(scrim)
    dark_arm, light_arm = (line for line in method.splitlines() if "=>" in line)
    assert "hsla_from_hex(0x050508)" in dark_arm
    assert "self.text()" in light_arm


def test_scrims_colour_resolving_to_session_is_refused() -> None:
    scrim = {"dark": {"colour": "session", "opacity": 0.60}, "light": {"colour": "#fff", "opacity": 0.28}}
    with pytest.raises(ValueError, match="session"):
        render_scrim(scrim)


def test_status_ring_track_colour_delegates_to_the_colour_it_names() -> None:
    sizes = {"status.ring": {"diameter": 16, "thickness": 2, "unit": "px", "track_colour": {"ref": "track"}}}
    _, methods = render_sizes({"sizes": sizes})
    (method,) = methods
    assert "status_ring_track" in method
    assert "self.track()" in method


def test_focus_ring_colour_is_the_themes_own_session_field_not_a_delegate() -> None:
    # focus_ring.colour is "session" directly (not a ref to a named colour), so there
    # is nothing to delegate to - it reads the Theme's own session field.
    data = {"focus_ring": {"width": 2, "unit": "px", "offset": 2, "colour": "session", "corner_radius_addition": 4}}
    _, method = render_focus_ring(data)
    assert method == "pub fn focus_ring_colour(&self) -> Hsla {\n        self.session\n    }"


def test_a_ramp_stop_delegates_to_the_colour_it_names() -> None:
    data = {"ramps": [{"name": "ramp.extended", "use": "x", "stops": [{"colour": {"ref": "ramp.1 violet"}, "stop": 0}]}]}
    (method,) = render_ramps(data)
    assert "self.ramp_1_violet()" in method


# ---------------------------------------------------------------------------
# render_plain_group: a single-field group's bare const never writes a redundant
# 'static (clippy's redundant_static_lifetimes refuses it on a const's own type,
# which is implicitly 'static already - a struct field, with more than one field,
# still needs it spelled out)
# ---------------------------------------------------------------------------


def test_a_single_string_field_consts_type_has_no_redundant_static() -> None:
    rust = render_plain_group("palette.input", {"family": "system"}, {})
    assert "'static" not in rust
    assert "pub const PALETTE_INPUT: &str" in rust


def test_a_single_list_of_strings_fields_const_has_no_redundant_static() -> None:
    rust = render_plain_group("font.terminal", {"family": ["JetBrains Mono", "Menlo"]}, {})
    assert "'static" not in rust
    assert 'pub const FONT_TERMINAL_DESCRIPTION: &str = "JetBrains Mono, Menlo";' in rust


def test_a_multi_field_structs_own_fields_keep_their_static() -> None:
    rust = render_plain_group("font.ui", {"family": "system", "role": "ui"}, {})
    assert "pub family: &'static str," in rust


# ---------------------------------------------------------------------------
# render: determinism - the plan's own commitment, proven on the smallest fixture
# that exercises every section render() requires outright (the rest it reads with
# .get(), so they're free to omit here)
# ---------------------------------------------------------------------------

MINIMAL_FIXTURE = {
    "artifact_version": "test-version",
    "colours": [],
    "tint": {"name": "tint", "colour": "session", "opacity": 0.1, "use": "x"},
    "sizes": {},
    "focus_ring": {"width": 2, "unit": "px", "offset": 2, "colour": "session"},
}


def test_render_is_deterministic_for_the_same_input() -> None:
    assert render(MINIMAL_FIXTURE) == render(MINIMAL_FIXTURE)


def test_render_produces_text_for_every_required_section() -> None:
    rendered = render(MINIMAL_FIXTURE)
    assert "test-version" in rendered
    assert "pub fn tint" in rendered
    assert "pub fn focus_ring_colour" in rendered


# ---------------------------------------------------------------------------
# a generated struct derives Debug/Clone/Copy (handy for anyone constructing a test
# fixture against it) and, separately, a `_DESCRIPTION` const in the design's own
# words - the gallery's "Other named values" section reads *that*, not `{:?}` on the
# struct: a Rust debug dump of nested braces and field names isn't what a reviewer
# matches against the design's own page (team lead's review of the first capture)
# ---------------------------------------------------------------------------


def test_a_multi_field_groups_struct_derives_debug_clone_copy() -> None:
    rust = render_plain_group("font.ui", {"family": "system", "role": "ui"}, {})
    assert "#[derive(Debug, Clone, Copy)]\npub struct FontUi" in rust


def test_a_multi_field_groups_description_is_the_designs_own_words_not_rust_syntax() -> None:
    rust = render_plain_group("nav.side", {"width": 232, "row_height": 30, "tab_indent": 28, "unit": "px"}, {})
    assert 'pub const NAV_SIDE_DESCRIPTION: &str = "width 232px, row_height 30px, tab_indent 28px";' in rust
    assert "{" not in rust.split("NAV_SIDE_DESCRIPTION")[1]


def test_a_single_field_groups_description_is_just_its_own_value() -> None:
    rust = render_plain_group("pane.caption", {"height": 30, "unit": "px"}, {})
    assert 'pub const PANE_CAPTION_DESCRIPTION: &str = "30px";' in rust


def test_a_whole_number_description_has_no_trailing_zero() -> None:
    rust = render_plain_group("pane.caption", {"height": 30, "unit": "px"}, {})
    assert "30.0px" not in rust
    assert "30px" in rust


# ---------------------------------------------------------------------------
# the regenerate-and-diff logic check-foundations uses
# ---------------------------------------------------------------------------


def test_matching_text_is_not_stale() -> None:
    assert is_stale(rendered="a", existing="a") is False


def test_differing_text_is_stale() -> None:
    assert is_stale(rendered="a", existing="b") is True


def test_the_staleness_message_names_the_fix() -> None:
    assert "just generate-foundations" in staleness_message()
