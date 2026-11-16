"""`ui_colours`'s pure functions against plain data and source fixtures (tests/adapter/ reads the real crate)."""

from steg_tools.rust_source import RustFile
from steg_tools.ui_colours import Literal, Violation, format_violation, is_exempt, literal_colours, violations

SURFACE_RAISED_INDEX = {"1c1b21": (("surface.raised", "dark"),)}
COINCIDING_INDEX = {"3d3b45": (("outline", "dark"), ("track", "dark"))}


# --- is_exempt -------------------------------------------------------------------------


def test_is_exempt_for_a_foundations_path() -> None:
    assert is_exempt("crates/ui/src/foundations/generated.rs") is True


def test_is_exempt_for_the_gallery_path() -> None:
    assert is_exempt("crates/ui/gallery/main.rs") is True


def test_is_exempt_is_false_for_an_ordinary_atom_path() -> None:
    assert is_exempt("crates/ui/src/atoms/button.rs") is False


# --- literal_colours: one case per literal form, and its non-literal twin --------------


def test_literal_colours_catches_a_bare_hex_integer() -> None:
    found = literal_colours("fn f() -> u32 { 0x1c1b21 }\n")
    assert found == (Literal(1, "0x1c1b21", "1c1b21"),)


def test_literal_colours_passes_a_short_hex_integer_that_is_not_shaped_like_a_colour() -> None:
    # A 2-digit hex (a bitmask, a flag) is not flagged - an accepted under-reach, not a bug.
    assert literal_colours("fn f() -> u8 { 0x3F }\n") == ()


def test_literal_colours_catches_a_hex_colour_string() -> None:
    found = literal_colours('fn f() -> &\'static str { "#1c1b21" }\n')
    assert found == (Literal(1, '"#1c1b21"', "1c1b21"),)


def test_literal_colours_passes_an_ordinary_string() -> None:
    assert literal_colours('fn f() -> &\'static str { "hello" }\n') == ()


def test_literal_colours_catches_a_named_constructor_call() -> None:
    found = literal_colours("fn f() { rgb(0x1c1b21); }\n")
    assert found == (Literal(1, "rgb(0x1c1b21)", None),)


def test_literal_colours_catches_a_qualified_named_constructor_call() -> None:
    found = literal_colours("fn f() { gpui::black(); }\n")
    assert found == (Literal(1, "gpui::black()", None),)


def test_literal_colours_passes_a_call_not_in_the_constructor_set() -> None:
    assert literal_colours("fn f() { some_colour.opacity(0.5); }\n") == ()


def test_literal_colours_catches_an_hsla_struct_literal() -> None:
    found = literal_colours("fn f() { let _ = Hsla { h: 0.5, s: 0.5, l: 0.5, a: 1.0 }; }\n")
    assert found == (Literal(1, "Hsla { h: 0.5, s: 0.5, l: 0.5, a: 1.0 }", None),)


def test_literal_colours_catches_a_qualified_rgba_struct_literal() -> None:
    found = literal_colours("fn f() { let _ = gpui::Rgba { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }; }\n")
    assert found == (Literal(1, "gpui::Rgba { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }", None),)


def test_literal_colours_passes_an_unrelated_struct_literal() -> None:
    assert literal_colours("fn f() { let _ = Point { x: 1.0, y: 2.0 }; }\n") == ()


def test_literal_colours_does_not_double_count_a_hex_argument_already_inside_a_constructor_call() -> None:
    # One mistake (rgb(0x1c1b21)), not two findings - the call already names it.
    found = literal_colours("fn f() { rgb(0x1c1b21); }\n")
    assert len(found) == 1


def test_literal_colours_still_catches_a_bare_hex_that_is_never_wrapped_in_a_call() -> None:
    # gallery/main.rs's own shape: a helper returns a bare u32, passed to rgb() elsewhere,
    # not visible to this file at all - caught at its own definition.
    found = literal_colours("fn placeholder_colour() -> u32 { 0x1c1b21 }\n")
    assert found == (Literal(1, "0x1c1b21", "1c1b21"),)


# --- violations: the exemption, as its own mechanism ------------------------------------


def test_violations_skips_a_literal_inside_the_foundations_path() -> None:
    file = RustFile("crates/ui/src/foundations/generated.rs", "pub const X: u32 = 0x1c1b21;\n")
    assert violations((file,), SURFACE_RAISED_INDEX) == ()


def test_violations_skips_a_literal_inside_the_gallery_path() -> None:
    file = RustFile("crates/ui/gallery/main.rs", "fn f() -> u32 { 0x1c1b21 }\n")
    assert violations((file,), SURFACE_RAISED_INDEX) == ()


def test_violations_reports_the_same_literal_outside_either_exempt_path() -> None:
    file = RustFile("crates/ui/src/atoms/button.rs", "fn f() -> u32 { 0x1c1b21 }\n")
    found = violations((file,), SURFACE_RAISED_INDEX)
    assert found == (Violation(file.path, 1, "0x1c1b21", (("surface.raised", "dark"),)),)


def test_violations_lists_every_matching_name_when_more_than_one_matches() -> None:
    file = RustFile("crates/ui/src/atoms/button.rs", "fn f() -> u32 { 0x3d3b45 }\n")
    found = violations((file,), COINCIDING_INDEX)
    assert found[0].matches == (("outline", "dark"), ("track", "dark"))


def test_violations_has_no_matches_when_the_literal_matches_no_foundations_value() -> None:
    file = RustFile("crates/ui/src/atoms/button.rs", "fn f() -> u32 { 0xabcdef }\n")
    found = violations((file,), SURFACE_RAISED_INDEX)
    assert found == (Violation(file.path, 1, "0xabcdef", ()),)


# --- formatting --------------------------------------------------------------------------


def test_format_violation_names_one_matching_foundations_value() -> None:
    message = format_violation(Violation("crates/ui/src/atoms/button.rs", 5, "0x1c1b21", (("surface.raised", "dark"),)))
    assert "R120" in message
    assert "surface.raised" in message
    assert "dark" in message


def test_format_violation_lists_two_matching_names() -> None:
    message = format_violation(Violation("crates/ui/src/atoms/button.rs", 5, "0x3d3b45", (("outline", "dark"), ("track", "dark"))))
    assert "outline" in message
    assert "track" in message


def test_format_violation_says_plainly_when_nothing_matches() -> None:
    message = format_violation(Violation("crates/ui/src/atoms/button.rs", 5, "0xabcdef", ()))
    assert "No foundations value" in message
