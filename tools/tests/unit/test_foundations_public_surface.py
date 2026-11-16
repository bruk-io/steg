"""The R123 compile-level guarantee check (review #5 of TASK-1.93): a `pub fn ... -> Hsla`
outside `impl Theme` is a violation - the check that would have caught
`default_session(Appearance) -> Hsla` before it shipped."""

from steg_tools.foundations_public_surface import (
    Violation,
    bare_hsla_returning_functions,
    format_violation,
)


def test_a_bare_hsla_returning_pub_fn_outside_impl_theme_is_a_violation() -> None:
    source = "pub fn default_session(appearance: Appearance) -> Hsla {\n    gpui::black()\n}\n"
    found = bare_hsla_returning_functions("mod.rs", source)
    assert found == (Violation("mod.rs", "default_session"),)


def test_a_pub_fn_returning_hsla_inside_impl_theme_is_not_a_violation() -> None:
    source = "impl Theme {\n    pub fn text(&self) -> Hsla {\n        hsla_from_hex(0x000000)\n    }\n}\n"
    assert bare_hsla_returning_functions("generated.rs", source) == ()


def test_a_pub_fn_returning_something_else_entirely_is_not_a_violation() -> None:
    # hex_of/tint/scrim-shaped: Hsla is a field, not the bare return type.
    source = "pub fn hex_of(colour: Hsla) -> String {\n    String::new()\n}\n"
    assert bare_hsla_returning_functions("mod.rs", source) == ()


def test_a_private_fn_returning_hsla_outside_impl_theme_is_not_a_violation() -> None:
    # generated.rs's own per-colour fns (fn surface_ground_dark() -> Hsla) are private.
    source = "fn surface_ground_dark() -> Hsla {\n    hsla_from_hex(0x0e0d10)\n}\n"
    assert bare_hsla_returning_functions("generated.rs", source) == ()


def test_two_impl_theme_blocks_each_shelter_their_own_methods() -> None:
    source = (
        "impl Theme {\n    pub fn text(&self) -> Hsla {\n        a()\n    }\n}\n"
        "pub struct Other;\n"
        "impl Theme {\n    pub fn track(&self) -> Hsla {\n        b()\n    }\n}\n"
    )
    assert bare_hsla_returning_functions("generated.rs", source) == ()


def test_a_violation_between_two_impl_theme_blocks_is_still_caught() -> None:
    source = (
        "impl Theme {\n    pub fn text(&self) -> Hsla {\n        a()\n    }\n}\n"
        "pub fn stray(appearance: Appearance) -> Hsla {\n    c()\n}\n"
        "impl Theme {\n    pub fn track(&self) -> Hsla {\n        b()\n    }\n}\n"
    )
    assert bare_hsla_returning_functions("generated.rs", source) == (Violation("generated.rs", "stray"),)


def test_format_violation_names_the_file_the_function_and_r123() -> None:
    message = format_violation(Violation("mod.rs", "default_session"))
    assert "mod.rs" in message
    assert "default_session" in message
    assert "R123" in message
