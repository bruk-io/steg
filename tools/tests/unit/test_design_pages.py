"""`steg_tools.design_pages`'s parsing and comparison logic, pure, no doubles, no disk."""

from steg_tools.design_pages import (
    REVERSE_ALLOW_LIST,
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
    parse_page,
    reverse_missing,
    sentence_mismatches,
    shape_problems,
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
    # "Tokens.dc.html" is expected to show >=17 colour rows; two is a refusal, not a pass.
    page = parse_page(page_html(COLOUR_TABLE))
    problems = shape_problems({"Tokens.dc.html": page, "Details.dc.html": page})
    assert any("Tokens.dc.html" in p and "colours" in p for p in problems)


def test_shape_problems_is_empty_when_every_minimum_is_met() -> None:
    colours = tuple(ColourRow(f"c{i}", "#000000", "#ffffff") for i in range(17))
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


def test_tier_a_the_track_colour_is_never_checked_even_when_a_file_entry_names_it() -> None:
    from steg_tools.design_pages import Page

    page = Page((), (), ())
    found = colour_mismatches(({"name": "track", "dark": "#3d3b45", "light": "#cfcbc5"},), page, "Tokens.dc.html")
    assert found == ()


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


def test_tier_b_a_ref_object_contributes_no_number_to_compare() -> None:
    # status.ring.track_colour is {"ref": "track"} - no int/float leaf, so it never demands a
    # number the page's prose can't give it.
    from steg_tools.design_pages import Page

    page = Page((), (SentenceRow("status.ring", "16px across, 2px thick; grey track"),), ())
    value = {"diameter": 16, "thickness": 2, "unit": "px", "track_colour": {"ref": "track"}}
    found = sentence_mismatches({}, {"status.ring": value}, page, "Tokens.dc.html")
    assert found == ()


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


def test_reverse_a_name_on_the_allow_list_is_not_reported() -> None:
    from steg_tools.design_pages import Page

    assert "radius" in REVERSE_ALLOW_LIST
    page = Page((), (SentenceRow("radius", "14px panes and palette; 10px tab track; 8px inside them"),), ())
    found = reverse_missing(frozenset(), frozenset(), page, "Tokens.dc.html")
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
