"""A just recipe's name starts with a verb on the allowed list."""

from steg_tools.recipes import Recipe, first_word, format_offender, offenders, recipes_from_dump

VERBS = frozenset({"check", "test", "build"})


def test_first_word_is_the_leading_hyphen_separated_segment() -> None:
    assert first_word("check-boundaries") == "check"
    assert first_word("test") == "test"


def test_a_verb_first_recipe_is_not_an_offender() -> None:
    assert offenders((Recipe("check-boundaries", False),), VERBS) == ()


def test_a_noun_first_recipe_is_an_offender() -> None:
    assert offenders((Recipe("packages", False),), VERBS) == (Recipe("packages", False),)


def test_a_private_recipe_is_never_an_offender_whatever_its_name() -> None:
    assert offenders((Recipe("default", True),), VERBS) == ()
    assert offenders((Recipe("packages", True),), VERBS) == ()


def test_more_than_one_offender_is_each_reported() -> None:
    recipes = (Recipe("check", False), Recipe("packages", False), Recipe("affected", False))
    assert offenders(recipes, VERBS) == (Recipe("packages", False), Recipe("affected", False))


def test_format_offender_names_the_recipe_the_word_and_the_file() -> None:
    message = format_offender(Recipe("packages", False))
    assert "just packages" in message
    assert "'packages'" in message
    assert "tools/src/steg_tools/recipes.py" in message


def test_recipes_from_dump_parses_the_shape_just_dump_produces() -> None:
    dump = {
        "recipes": {
            "default": {"private": True},
            "check": {"private": False},
            "check-boundaries": {"private": False},
        }
    }
    assert recipes_from_dump(dump) == (
        Recipe("default", True),
        Recipe("check", False),
        Recipe("check-boundaries", False),
    )
