"""`steg_tools.design_pages`'s parsing and comparison logic, pure, no doubles, no disk."""

from steg_tools.design_pages import (
    RADIUS_MAP,
    REVERSE_ALLOW_LIST,
    SKIPPED_COLOURS,
    TIER_B_NAMES,
    TIER_C_SKIPS,
    AnsiRow,
    ColourRow,
    Mismatch,
    Missing,
    SentenceRow,
    Skipped,
    ansi_mismatches,
    ansi_reverse_missing,
    colour_mismatches,
    mapped_sentence_mismatches,
    parse_page,
    reverse_missing,
    sentence_mismatches,
    shape_problems,
    unaccounted_names,
)

HELMET = "</helmet>"
SCRIPT_TAIL = '<script type="text/x-dc" data-dc-script data-props=\'{}\'>var off = "#5f5d69";</script>'


def page_html(body: str) -> str:
    """Wrap a body fragment the way a real saved page wraps it: a `</helmet>` boundary before
    it, and the page's own script block (never read as page content) after it.
    """
    return f"<html><head></head><x-dc><helmet><link/></helmet>{body}</x-dc>{SCRIPT_TAIL}</html>"


COLOUR_TABLE = """
<div style="display: flex; flex-direction: column; gap: 8px">
<div style="display: grid"><span>Name</span><span>Dark</span><span>Light</span><span>Used for</span></div>
<div style="display: grid"><span>surface.ground</span><span><span aria-hidden="true" style="background:#0e0d10"></span><span>#0e0d10</span></span><span><span aria-hidden="true" style="background:#f1efec"></span><span>#f1efec</span></span><span>The window</span></div>
<div style="display: grid"><span>surface.pane</span><span><span aria-hidden="true" style="background:#141318"></span><span>#141318</span></span><span><span aria-hidden="true" style="background:#f8f6f4"></span><span>#f8f6f4</span></span><span>Panes</span></div>
</div>
"""

SENTENCE_TABLE = """
<div style="display: flex; flex-direction: column; gap: 8px">
<div style="display: grid"><span>Name</span><span>Value</span></div>
<div style="display: grid"><span>space</span><span>Steps of 2, 4, 6, 8, 10, 12, 16</span></div>
<div style="display: grid"><span>nav.side</span><span>232px wide; rows 30px, tabs indented 28px</span></div>
</div>
"""


def ansi_section(theme: str, cards: str) -> str:
    return f'<section><h2>Terminal theme: {theme}</h2><div style="display: grid">{cards}</div></section>'


def ansi_card(name: str, hexcode: str) -> str:
    return f'<div><span aria-hidden="true"></span><span>{name}</span><span>{hexcode}</span></div>'


def test_a_4_column_fragment_parses_to_colour_rows() -> None:
    page = parse_page(page_html(COLOUR_TABLE))
    assert page.colours == (
        ColourRow("surface.ground", "#0e0d10", "#f1efec"),
        ColourRow("surface.pane", "#141318", "#f8f6f4"),
    )


def test_a_2_column_fragment_parses_to_sentence_rows() -> None:
    page = parse_page(page_html(SENTENCE_TABLE))
    assert page.sentences == (
        SentenceRow("space", "Steps of 2, 4, 6, 8, 10, 12, 16"),
        SentenceRow("nav.side", "232px wide; rows 30px, tabs indented 28px"),
    )


def test_an_ansi_swatch_card_parses_to_ansi_rows_scoped_per_theme() -> None:
    body = (
        ansi_section("steg dark", ansi_card("black", "#2c2a33") + ansi_card("bright black", "#6f6d7a"))
        + ansi_section("steg light", ansi_card("black", "#1b1a1f"))
    )
    page = parse_page(page_html(body))
    assert page.ansi == (
        AnsiRow("steg dark", "black", "#2c2a33"),
        AnsiRow("steg dark", "bright black", "#6f6d7a"),
        AnsiRow("steg light", "black", "#1b1a1f"),
    )


def test_a_header_with_zero_data_rows_parses_to_an_empty_tuple() -> None:
    body = '<div style="display: flex; flex-direction: column"><div style="display: grid"><span>Name</span><span>Value</span></div></div>'
    page = parse_page(page_html(body))
    assert page.sentences == ()


def test_shape_problems_flags_a_page_under_its_asserted_minimum() -> None:
    # "Tokens.dc.html" is expected to show >=18 colour rows (17 plus track, added at
    # artifact version 1794796789-1b13); two is a refusal, not a pass.
    page = parse_page(page_html(COLOUR_TABLE))
    problems = shape_problems({"Tokens.dc.html": page, "Details.dc.html": page})
    assert any("Tokens.dc.html" in p and "colours" in p for p in problems)


def test_shape_problems_is_empty_when_every_minimum_is_met() -> None:
    colours = tuple(ColourRow(f"c{i}", "#000000", "#ffffff") for i in range(18))
    sentences = tuple(SentenceRow(f"s{i}", "x") for i in range(17))
    ansi = tuple(AnsiRow("t", f"a{i}", "#000000") for i in range(32))

    from steg_tools.design_pages import Page

    pages = {
        "Tokens.dc.html": Page(colours, sentences, ()),
        "Details.dc.html": Page((), (), ansi),
    }
    assert shape_problems(pages) == ()


def test_tier_a_matching_colour_is_no_mismatch() -> None:
    from steg_tools.design_pages import Page

    page = Page((ColourRow("surface.ground", "#0e0d10", "#f1efec"),), (), ())
    found = colour_mismatches(({"name": "surface.ground", "dark": "#0e0d10", "light": "#f1efec"},), page, "Tokens.dc.html")
    assert found == ()


def test_tier_a_a_changed_dark_hex_is_a_mismatch_naming_both_forms() -> None:
    from steg_tools.design_pages import Page

    page = Page((ColourRow("surface.ground", "#111111", "#f1efec"),), (), ())
    found = colour_mismatches(({"name": "surface.ground", "dark": "#0e0d10", "light": "#f1efec"},), page, "Tokens.dc.html")
    assert found == (Mismatch("surface.ground", "dark=#0e0d10 light=#f1efec", "dark=#111111 light=#f1efec", "Tokens.dc.html"),)


def test_tier_a_a_name_absent_from_the_page_is_a_mismatch() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), ())
    found = colour_mismatches(({"name": "surface.ground", "dark": "#0e0d10", "light": "#f1efec"},), page, "Tokens.dc.html")
    assert len(found) == 1
    assert found[0].page_form == "<no row found>"


def test_tier_a_track_is_checked_like_any_other_colour_no_exception_left() -> None:
    # Closed at artifact version 1794796789-1b13: track gained its own row, so it is no
    # longer in SKIPPED_COLOURS and is checked exactly like every other colour.
    from steg_tools.design_pages import Page

    assert "track" not in SKIPPED_COLOURS
    page = Page((ColourRow("track", "#3d3b45", "#cfcbc5"),), (), ())
    found = colour_mismatches(({"name": "track", "dark": "#3d3b45", "light": "#cfcbc5"},), page, "Tokens.dc.html")
    assert found == ()


def test_tier_a_a_changed_track_colour_is_a_mismatch() -> None:
    from steg_tools.design_pages import Page

    page = Page((ColourRow("track", "#000000", "#cfcbc5"),), (), ())
    found = colour_mismatches(({"name": "track", "dark": "#3d3b45", "light": "#cfcbc5"},), page, "Tokens.dc.html")
    assert found == (Mismatch("track", "dark=#3d3b45 light=#cfcbc5", "dark=#000000 light=#cfcbc5", "Tokens.dc.html"),)


def test_tier_b_a_sentence_with_every_number_is_no_mismatch() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("nav.side", "232px wide; rows 30px, tabs indented 28px"),), ())
    found = sentence_mismatches({}, {"nav.side": {"width": 232, "row_height": 30, "tab_indent": 28, "unit": "px"}}, page, "Tokens.dc.html")
    assert found == ()


def test_tier_b_a_missing_number_is_a_mismatch() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("nav.side", "232px wide; rows 30px"),), ())
    found = sentence_mismatches({}, {"nav.side": {"width": 232, "row_height": 30, "tab_indent": 28, "unit": "px"}}, page, "Tokens.dc.html")
    assert len(found) == 1
    assert found[0].name == "nav.side"


def test_tier_b_an_opacity_field_is_compared_as_a_whole_number_percent() -> None:
    # sizes.scrim: the file holds a fraction (0.60/0.28); the page always writes it as a
    # whole-number percent ("60%"/"28%") - the one documented exception to "compare the
    # number as written" (TASK-1.96's plan, Tier B).
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("scrim", "#050508 at 60% in dark, ink at 28% in light"),), ())
    value = {"dark": {"colour": "#050508", "opacity": 0.60}, "light": {"colour": {"ref": "text"}, "opacity": 0.28}}
    found = sentence_mismatches({}, {"scrim": value}, page, "Tokens.dc.html")
    assert found == ()


def test_tier_b_a_name_with_no_numbers_in_the_file_degrades_to_name_presence_only() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("font.ui", "anything at all"),), ())
    found = sentence_mismatches({"font.ui": {"family": "system", "role": "ui"}}, {}, page, "Tokens.dc.html")
    assert found == ()


TAB_WIDTH_ROW_TEXT = "120px usual, 240px at most, 72px at least"


def test_field_word_map_tab_width_passes_against_the_unswapped_row() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("tab.width", TAB_WIDTH_ROW_TEXT),), ())
    value = {"usual": 120, "max": 240, "min": 72, "unit": "px"}
    assert sentence_mismatches({}, {"tab.width": value}, page, "Tokens.dc.html") == ()


def test_field_word_map_catches_the_swap_the_weak_check_alone_cannot() -> None:
    # The review's own negative control: usual and min swapped, same two numbers still
    # present on the page - the plain "every number is present somewhere" check alone
    # (sentence_mismatches without FIELD_WORD_MAPS) would pass this; the tightened check
    # must not.
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("tab.width", TAB_WIDTH_ROW_TEXT),), ())
    swapped = {"usual": 72, "max": 240, "min": 120, "unit": "px"}
    found = sentence_mismatches({}, {"tab.width": swapped}, page, "Tokens.dc.html")
    names = {mismatch.name for mismatch in found}
    assert "tab.width.usual" in names
    assert "tab.width.min" in names
    assert "tab.width.max" not in names  # unswapped, still ties to "at most" correctly


def test_field_word_map_falls_back_when_the_sentence_gives_no_word() -> None:
    # nav.side.row_height has no entry in FIELD_WORD_MAPS (its clause's word, "rows",
    # precedes the number instead of following it) - a corruption there is invisible to
    # the tightened check, exactly the documented fallback, not a bug.
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("nav.side", "232px wide; rows 30px, tabs indented 28px"),), ())
    value = {"width": 232, "row_height": 999, "tab_indent": 28, "unit": "px"}
    # The weak set check *does* still catch this, since 999 is absent from the page
    # entirely - this proves the fallback is to the weak check, not to silence.
    found = sentence_mismatches({}, {"nav.side": value}, page, "Tokens.dc.html")
    assert len(found) == 1
    assert found[0].name == "nav.side"


def test_tier_b_a_ref_object_contributes_no_number_to_compare() -> None:
    # status.ring.track_colour is {"ref": "track"} - no int/float leaf, so it never demands a
    # number the page's prose can't give it.
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("status.ring", "16px across, 2px thick; grey track"),), ())
    value = {"diameter": 16, "thickness": 2, "unit": "px", "track_colour": {"ref": "track"}}
    found = sentence_mismatches({}, {"status.ring": value}, page, "Tokens.dc.html")
    assert found == ()


RADIUS_ROW_TEXT = "14px panes and palette; 10px tab track and session chips; 8px inside them"


def test_radius_map_the_three_mapped_entries_pass_against_the_merged_row() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("radius", RADIUS_ROW_TEXT),), ())
    file_sizes = {
        "radius.surface": {"value": 14, "unit": "px", "use": "Panes and the palette"},
        "radius.track": {"value": 10, "unit": "px", "use": "The tab track and session chips"},
        "radius.inner": {"value": 8, "unit": "px", "use": "Anything inside one of those"},
    }
    assert mapped_sentence_mismatches(file_sizes, RADIUS_MAP, page, "Tokens.dc.html") == ()


def test_radius_map_a_changed_corner_in_a_scratch_copy_is_refused() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("radius", RADIUS_ROW_TEXT),), ())
    file_sizes = {"radius.surface": {"value": 99, "unit": "px"}}
    found = mapped_sentence_mismatches(file_sizes, RADIUS_MAP, page, "Tokens.dc.html")
    # Both the weak set check and RADIUS_WORD_MAP's tightened check fire - correctly, 99
    # is both absent from the page's number set and not tied to "palette".
    assert len(found) == 2
    assert all(mismatch.name == "radius.surface" for mismatch in found)
    assert found[0].page_form == RADIUS_ROW_TEXT


def test_radius_map_a_page_row_absent_is_a_mismatch() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), ())
    found = mapped_sentence_mismatches({"radius.surface": {"value": 14, "unit": "px"}}, RADIUS_MAP, page, "Tokens.dc.html")
    assert len(found) == 1
    assert found[0].page_form == "<no row found>"


def test_the_fully_round_corner_is_not_in_the_map_and_stays_declared_unseen() -> None:
    assert "radius.round" not in RADIUS_MAP
    assert any(skip.name == ("sizes", "radius.round") for skip in TIER_C_SKIPS)
    assert not any(skip.name[:2] == ("sizes", "radius.surface") for skip in TIER_C_SKIPS)
    assert not any(skip.name[:2] == ("sizes", "radius.track") for skip in TIER_C_SKIPS)
    assert not any(skip.name[:2] == ("sizes", "radius.inner") for skip in TIER_C_SKIPS)


def test_ansi_mismatch_is_scoped_to_its_own_theme() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), (AnsiRow("steg dark", "black", "#2c2a33"), AnsiRow("steg light", "black", "#1b1a1f")))
    themes = (
        {"name": "steg dark", "ansi": {"black": "#2c2a33"}},
        {"name": "steg light", "ansi": {"black": "#1b1a1f"}},
    )
    assert ansi_mismatches(themes, page, "Details.dc.html") == ()


def test_ansi_mismatch_translates_underscore_to_the_pages_space() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), (AnsiRow("steg dark", "bright black", "#6f6d7a"),))
    themes = ({"name": "steg dark", "ansi": {"bright_black": "#6f6d7a"}},)
    assert ansi_mismatches(themes, page, "Details.dc.html") == ()


def test_ansi_mismatch_catches_a_changed_hex() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), (AnsiRow("steg dark", "black", "#000001"),))
    themes = ({"name": "steg dark", "ansi": {"black": "#2c2a33"}},)
    found = ansi_mismatches(themes, page, "Details.dc.html")
    assert found == (Mismatch("steg dark.black", "#2c2a33", "#000001", "Details.dc.html"),)


def test_reverse_a_page_name_absent_from_the_file_is_reported_missing() -> None:
    from steg_tools.design_pages import Page

    page = Page((ColourRow("ghost", "#000000", "#ffffff"),), (), ())
    found = reverse_missing(frozenset(), frozenset(), page, "Tokens.dc.html")
    assert found == (Missing("ghost", "dark=#000000 light=#ffffff", "Tokens.dc.html"),)


def test_reverse_allow_list_is_empty_now_that_radius_is_matched_via_the_map() -> None:
    # "radius" was an allow-list entry until RADIUS_MAP gave it a real match instead of
    # an exemption - the allow list goes back to empty (see REVERSE_ALLOW_LIST's own
    # comment and the module docstring).
    assert REVERSE_ALLOW_LIST == frozenset()


def test_reverse_a_name_on_the_allow_list_is_not_reported() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("ghost", "text"),), ())
    found = reverse_missing(frozenset(), frozenset(), page, "Tokens.dc.html", allow=frozenset({"ghost"}))
    assert found == ()


def test_reverse_a_name_matched_via_the_radius_map_is_not_reported() -> None:
    # This is the real wiring main() uses: a name RADIUS_MAP explains is passed into
    # `allow`, same as a genuinely exempted name, but the two sets come from different
    # sources (RADIUS_MAP.values() vs REVERSE_ALLOW_LIST), not one hidden list.
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("radius", "14px panes and palette; 10px tab track; 8px inside them"),), ())
    found = reverse_missing(frozenset(), frozenset(), page, "Tokens.dc.html", allow=frozenset(RADIUS_MAP.values()))
    assert found == ()


def test_reverse_a_name_present_in_the_file_is_not_reported() -> None:
    from steg_tools.design_pages import Page

    page = Page((ColourRow("surface.ground", "#0e0d10", "#f1efec"),), (), ())
    found = reverse_missing(frozenset({"surface.ground"}), frozenset(), page, "Tokens.dc.html")
    assert found == ()


def test_ansi_reverse_a_page_name_absent_from_the_themes_ansi_is_reported() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), (AnsiRow("steg dark", "ghost", "#000000"),))
    themes = ({"name": "steg dark", "ansi": {"black": "#2c2a33"}},)
    found = ansi_reverse_missing(themes, page, "Details.dc.html")
    assert found == (Missing("steg dark.ghost", "#000000", "Details.dc.html"),)


def test_tier_c_every_declared_skip_has_a_non_empty_reason() -> None:
    assert len(TIER_C_SKIPS) > 0
    for skip in TIER_C_SKIPS:
        assert isinstance(skip, Skipped)
        assert skip.reason.strip() != ""


def test_tier_b_names_has_no_duplicate() -> None:
    assert len(TIER_B_NAMES) == len(set(TIER_B_NAMES))


def test_a_file_key_matched_on_more_than_one_page_is_a_refusal_not_first_match_wins() -> None:
    # Constructed, since in the real files this never happens (Layout.dc.html is never
    # parsed) - proves the ambiguity guard the plan promises stays in place.
    from steg_tools.design_pages import Page, ambiguous_matches

    pages = {
        "Tokens.dc.html": Page((), (SentenceRow("scrim", "#050508 at 60% in dark, ink at 28% in light"),), ()),
        "Other.dc.html": Page((), (SentenceRow("scrim", "something else entirely"),), ()),
    }
    found = ambiguous_matches("scrim", pages)
    assert found == ("Tokens.dc.html", "Other.dc.html")


def test_a_file_key_matched_on_only_one_page_is_not_ambiguous() -> None:
    from steg_tools.design_pages import Page, ambiguous_matches

    pages = {
        "Tokens.dc.html": Page((), (SentenceRow("scrim", "#050508 at 60% in dark, ink at 28% in light"),), ()),
        "Other.dc.html": Page((), (), ()),
    }
    assert ambiguous_matches("scrim", pages) == ()


MINIMAL_DATA = {
    "colours": [{"name": "track", "dark": "#000000", "light": "#ffffff", "use": "x"}],
    "colour_aliases": [{"name": "blocked", "ref": {"ref": "ramp.5 amber"}, "use": "x"}],
    "opacities": [{"name": "faint", "dark": 0.3, "light": 0.45, "use": "x"}],
    "ramps": [
        {"name": "ramp", "use": "x", "stops": [{"stop": 0}], "caption_opacity": {"dark": 0.5, "light": 0.4}},
        {"name": "ramp.extended", "use": "x", "stops": [{"stop": 0}]},
    ],
    "tint": {"name": "tint", "colour": "session", "opacity": 0.18, "use": "x"},
    "type": {"font.ui": {"family": "system", "role": "ui"}},
    "sizes": {"nav.side": {"width": 232, "unit": "px"}, "radius.surface": {"value": 14}, "radius.round": {"shape": "round"}},
    "motion": [{"name": "motion.status-fade", "duration": 200, "unit": "ms", "use": "x"}],
    "themes": [
        {"name": "steg dark", "bg": "#1", "fg": "#2", "bg_inactive": {"ref": "surface.pane"}, "selection": "#3", "cursor": "session", "ansi": {"black": "#4"}},
        {"name": "steg light", "bg": "#5", "fg": "#6", "bg_inactive": {"ref": "surface.pane"}, "selection": "#7", "cursor": "session", "ansi": {"black": "#8"}},
    ],
    "focus_ring": {"width": 2, "offset": 2, "colour": "session", "corner_radius_addition": 4},
    "session_colour": "session",
}

MINIMAL_SKIPPED = frozenset(
    {
        ("colour_aliases", "blocked", "ref"),
        ("opacities", "faint", "dark"),
        ("opacities", "faint", "light"),
        ("ramps", "ramp", "stops"),
        ("ramps", "ramp", "caption_opacity"),
        ("ramps", "ramp.extended", "stops"),
        ("tint",),
        ("sizes", "radius.round"),
        ("motion", "motion.status-fade", "duration"),
        ("themes", "steg dark", "bg"),
        ("themes", "steg dark", "fg"),
        ("themes", "steg dark", "bg_inactive"),
        ("themes", "steg dark", "selection"),
        ("themes", "steg dark", "cursor"),
        ("themes", "steg light", "bg"),
        ("themes", "steg light", "fg"),
        ("themes", "steg light", "bg_inactive"),
        ("themes", "steg light", "selection"),
        ("themes", "steg light", "cursor"),
        ("focus_ring", "width"),
        ("focus_ring", "offset"),
        ("focus_ring", "colour"),
        ("focus_ring", "corner_radius_addition"),
        ("session_colour",),
    }
)

MINIMAL_CHECKED = frozenset({"font.ui", "nav.side", "radius.surface"})


def test_unaccounted_names_is_empty_when_every_leaf_key_is_accounted_for() -> None:
    assert unaccounted_names(MINIMAL_DATA, skipped=MINIMAL_SKIPPED, checked=MINIMAL_CHECKED) == ()


def test_unaccounted_names_flags_a_key_on_neither_list() -> None:
    data = {**MINIMAL_DATA, "sizes": {**MINIMAL_DATA["sizes"], "a.new.size": {"value": 5, "unit": "px"}}}
    found = unaccounted_names(data, skipped=MINIMAL_SKIPPED, checked=MINIMAL_CHECKED)
    assert found == ("sizes.a.new.size.value",)


def test_unaccounted_names_an_entirely_empty_value_has_nothing_to_account_for() -> None:
    # A new key whose value is an empty dict or list has no leaves at all - vacuously
    # accounted for, not a false positive on the container key itself. Real data never
    # does this (every entry has at least one scalar or a `name`), but the walker's own
    # rule should hold regardless.
    data = {**MINIMAL_DATA, "sizes": {**MINIMAL_DATA["sizes"], "empty.size": {}}}
    assert unaccounted_names(data, skipped=MINIMAL_SKIPPED, checked=MINIMAL_CHECKED) == ()


def test_unaccounted_names_negative_control_removing_a_declared_unseen_entry_fails_naming_it() -> None:
    # The required negative control: a declared-unseen entry removed from `skipped` must
    # make that exact key show up as unaccounted, not fall through silently.
    without_focus_ring_width = MINIMAL_SKIPPED - {("focus_ring", "width")}
    found = unaccounted_names(MINIMAL_DATA, skipped=without_focus_ring_width, checked=MINIMAL_CHECKED)
    assert found == ("focus_ring.width",)


def test_unaccounted_names_a_dotted_key_is_never_mistaken_for_a_child_of_an_unrelated_name() -> None:
    # The review's own four reproductions, against the minimal fixture: a name that
    # merely shares a *string* prefix with a registered one (because a real JSON key can
    # itself contain a dot) must not be swallowed by it.
    data = {**MINIMAL_DATA, "sizes": {**MINIMAL_DATA["sizes"], "nav.side.extra": {"value": 1, "unit": "px"}}}
    found = unaccounted_names(data, skipped=MINIMAL_SKIPPED, checked=MINIMAL_CHECKED)
    assert found == ("sizes.nav.side.extra.value",)


def test_unaccounted_names_a_new_colour_field_nothing_checks_is_unaccounted() -> None:
    # A review finding: colours were registered as a whole-entry ancestor, so a stray
    # field nothing checks (only "dark"/"light" are) was silently swallowed.
    data = {**MINIMAL_DATA, "colours": [{**MINIMAL_DATA["colours"][0], "contrast": 1.5}]}
    found = unaccounted_names(data, skipped=MINIMAL_SKIPPED, checked=MINIMAL_CHECKED)
    assert found == ("colours.track.contrast",)


def test_unaccounted_names_default_arguments_cover_the_real_modules_own_registries() -> None:
    # Sanity check for the defaults themselves (TIER_C_SKIPS/TIER_B_NAMES/RADIUS_MAP), not
    # the real data file - the adapter test covers that against the real file on disk.
    assert unaccounted_names(MINIMAL_DATA) == ()
