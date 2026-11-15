"""`recipes_from_dump`/`offenders` against this repository's real `just --dump`, not a made-up fixture.

The unit suite in tests/unit/test_recipes.py proves the logic against JSON nobody has ever run
`just --dump` to produce. This is the other half: the real justfile, after the rename.
"""

from steg_tools import workspace
from steg_tools.recipes import ALLOWED_VERBS, offenders, recipes_from_dump


def real_dump() -> dict[str, object]:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    dump = workspace.just_dump(root)
    assert dump is not None, "just --dump must succeed for this test to mean anything"
    return dump


def test_the_real_justfile_has_no_recipe_named_against_the_verb_rule() -> None:
    assert offenders(recipes_from_dump(real_dump()), ALLOWED_VERBS) == ()


def test_the_real_justfile_leaves_no_alias_behind() -> None:
    # A separate, cheap mechanical proof that the rename didn't leave an alias for an old name.
    assert real_dump()["aliases"] == {}
