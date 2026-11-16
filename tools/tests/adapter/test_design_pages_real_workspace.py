"""`steg-design-check`'s CLI behaviour against the real repository: the one real boundary it has.

Reads the real `design/canvas/project/{Tokens,Details}.dc.html` always; `--data-file`
points the data-file half at a scratch copy for the negative controls, so the repository's
real pages stay untouched (per `team:testing`: real files, nothing mocked, only the input
this check exists to validate is swapped for a controlled one).
"""

import json
import shutil
from pathlib import Path

from steg_tools import design_pages
from steg_tools import workspace
from steg_tools.design_pages import RADIUS_MAP, TIER_B_NAMES, TIER_C_SKIPS, main, unaccounted_names


def real_data_path() -> Path:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    return root / "crates/ui/design/foundations.json"


def test_the_real_data_file_passes_against_the_real_saved_pages_today() -> None:
    assert main([]) == 0


def test_a_changed_dark_hex_in_a_scratch_copy_is_refused_naming_both_forms(tmp_path: Path, capsys: object) -> None:
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    for entry in data["colours"]:
        if entry["name"] == "surface.ground":
            assert entry["dark"] == "#0e0d10"
            entry["dark"] = "#123456"
    scratch.write_text(json.dumps(data))

    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "surface.ground" in captured.out
    assert "#123456" in captured.out
    assert "#0e0d10" in captured.out


def test_a_removed_colour_entry_in_a_scratch_copy_is_reported_missing(tmp_path: Path, capsys: object) -> None:
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    before = len(data["colours"])
    data["colours"] = [entry for entry in data["colours"] if entry["name"] != "surface.ground"]
    assert len(data["colours"]) == before - 1
    scratch.write_text(json.dumps(data))

    # The real pages still draw surface.ground - reported by the reverse direction, not
    # by Tier A (which only ever looks at names the file still has).
    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "surface.ground" in captured.out
    assert "no entry" in captured.out


def test_copying_the_real_file_byte_for_byte_still_passes(tmp_path: Path) -> None:
    # Sanity check for the two negative controls above: the harness itself (a bare
    # scratch copy, no mutation) must still pass, so a failure above is the mutation's
    # doing, not an artefact of reading from tmp_path.
    scratch = tmp_path / "foundations.json"
    shutil.copyfile(real_data_path(), scratch)
    assert main(["--data-file", str(scratch)]) == 0


def test_a_changed_corner_in_a_scratch_copy_is_refused(tmp_path: Path, capsys: object) -> None:
    # The engineer's small, reviewed map (RADIUS_MAP): sizes.radius.surface/track/inner
    # (14/10/8) each check their own number against Tokens.dc.html's one merged "radius"
    # row - this is the negative control proving the real data file's three mapped
    # entries do pass against the real page today (test_the_real_data_file_passes_
    # against_the_real_saved_pages_today above already proves the positive case).
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    assert data["sizes"]["radius.surface"]["value"] == 14
    data["sizes"]["radius.surface"]["value"] = 99
    scratch.write_text(json.dumps(data))

    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "radius.surface" in captured.out
    assert "99" in captured.out


def test_the_fully_round_corner_is_listed_as_unseen_in_the_tools_own_output(capsys: object) -> None:
    assert main([]) == 0
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "sizes.radius.round" in captured.out
    assert "sizes.radius.surface" not in captured.out  # no longer a declared exception


def test_swapping_tab_widths_usual_and_min_in_a_scratch_copy_is_refused(tmp_path: Path, capsys: object) -> None:
    # The reviewer's own negative control (TASK-1.96 review finding): the same two
    # numbers stay present on the page, swapped between two fields - a plain "every
    # number is present somewhere" check would miss this; FIELD_WORD_MAPS must not.
    scratch = tmp_path / "foundations.json"
    data = json.loads(real_data_path().read_text())
    tab_width = data["sizes"]["tab.width"]
    assert (tab_width["usual"], tab_width["min"]) == (120, 72)
    tab_width["usual"], tab_width["min"] = tab_width["min"], tab_width["usual"]
    scratch.write_text(json.dumps(data))

    assert main(["--data-file", str(scratch)]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "tab.width.usual" in captured.out
    assert "tab.width.min" in captured.out


def test_the_tools_passing_output_names_the_field_blind_caveat(capsys: object) -> None:
    assert main([]) == 0
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "field-blind" in captured.out


def test_every_leaf_key_of_the_real_data_file_is_checked_or_declared_unseen() -> None:
    data = json.loads(real_data_path().read_text())
    assert unaccounted_names(data) == ()


def test_removing_a_declared_unseen_entry_makes_that_key_unaccounted(tmp_path: Path) -> None:
    # Required negative control: a declared-unseen entry removed, the key it covered
    # must show up as unaccounted, not fall through silently.
    data = json.loads(real_data_path().read_text())
    skipped_without_one = frozenset(skip.name for skip in TIER_C_SKIPS) - {"session_colour"}
    found = unaccounted_names(data, skipped=skipped_without_one)
    assert found == ("session_colour",)


# --- Review finding (@bruk.r5): unaccounted_names must discover fields from the data
# itself at every level the file has, not from a fixed tuple written in the tool -
# `themes[]`/`focus_ring` were the two sections that did this wrong. One test per level
# the reviewer's own reproduction, and the team lead's broader ask, named: top level,
# inside `sizes` directly, inside a theme, inside `focus_ring`, and a new entry in a
# list. "Inside a size's group" (a new field added to an *existing* checked entry such
# as `nav.side`) is its own test below, with the reasoning for why it is not refused.


def test_a_new_top_level_key_is_refused(tmp_path: Path) -> None:
    data = json.loads(real_data_path().read_text())
    data["glow_colour"] = "#ffffff"
    assert unaccounted_names(data) == ("glow_colour",)


def test_a_new_key_directly_inside_sizes_is_refused(tmp_path: Path) -> None:
    data = json.loads(real_data_path().read_text())
    data["sizes"]["new_group"] = {"value": 5, "unit": "px"}
    assert unaccounted_names(data) == ("sizes.new_group.value",)


def test_a_new_field_inside_a_theme_is_refused_exactly_like_the_reviewers_reproduction() -> None:
    # The review's own reproduction (@bruk.r5): themes[0]["underline"] = "#fff" on a
    # scratch copy used to print "ok" and exit 0. Reproduced here against the real file,
    # not a fixture, so the fix is proven against what was actually broken.
    data = json.loads(real_data_path().read_text())
    assert data["themes"][0]["name"] == "steg dark"
    data["themes"][0]["underline"] = "#fff"
    assert unaccounted_names(data) == ("themes.steg dark.underline",)


def test_a_new_field_inside_focus_ring_is_refused_exactly_like_the_reviewers_reproduction() -> None:
    # The review's other reproduction: focus_ring["glow"] = 3.
    data = json.loads(real_data_path().read_text())
    data["focus_ring"]["glow"] = 3
    assert unaccounted_names(data) == ("focus_ring.glow",)


def test_a_new_entry_in_a_list_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    data["colour_aliases"].append({"name": "warning", "ref": {"ref": "ramp.5 amber"}, "use": "x"})
    found = unaccounted_names(data)
    assert "colour_aliases.warning.ref.ref" in found


def test_a_new_numeric_field_inside_an_existing_checked_group_is_accounted_for_by_its_ancestor() -> None:
    # Chosen reading of "inside a size's group" (ambiguous in the review's paraphrase;
    # the review's own literal reproduction only covers themes[]/focus_ring, tested
    # above). sizes.nav.side is already a covered whole value (TIER_B_NAMES); a new
    # *numeric* field added inside it is accounted for by that covering name, because
    # sentence_mismatches' _numbers_in already recurses the whole value and would check
    # this number for presence on the page - nothing numeric there goes uninspected. Not
    # the same as "undetectable": see the next test for the gap this leaves.
    data = json.loads(real_data_path().read_text())
    data["sizes"]["nav.side"]["new_measurement"] = 999
    assert unaccounted_names(data) == ()


def test_a_new_non_numeric_field_inside_an_existing_checked_group_is_a_known_accepted_gap() -> None:
    # The flip side of the test above, stated so it is a documented limit, not a silent
    # one: a non-numeric field inside an already-covered group is not inspected by
    # _numbers_in either, and unaccounted_names does not catch it (it is still "inside"
    # a covering ancestor). crates/ui/design/README.md says this plainly.
    data = json.loads(real_data_path().read_text())
    data["sizes"]["nav.side"]["alias"] = "a string _numbers_in never looks at"
    assert unaccounted_names(data) == ()


def _real_leaves_and_registry() -> tuple[list[str], frozenset[str], frozenset[str]]:
    """The real file's leaf paths, its skipped-name registry, and its full-path checked
    registry (type/sizes bare names translated to their section, the same way
    `unaccounted_names` does it) - shared by the property-style tests below.
    """
    data = json.loads(real_data_path().read_text())
    leaves = design_pages._leaf_paths(data, "")  # noqa: SLF001 - white-box test of the walker
    skipped = frozenset(skip.name for skip in TIER_C_SKIPS)
    checked_full = set()
    for bare in TIER_B_NAMES:
        if bare in data.get("type", {}):
            checked_full.add(f"type.{bare}")
        if bare in data.get("sizes", {}):
            checked_full.add(f"sizes.{bare}")
    for bare in RADIUS_MAP:
        if bare in data.get("sizes", {}):
            checked_full.add(f"sizes.{bare}")
    for colour in data["colours"]:
        checked_full.add(f"colours.{colour['name']}")
    return leaves, skipped, frozenset(checked_full)


def _covers(leaf: str, name: str) -> bool:
    return leaf == name or leaf.startswith(f"{name}.")


def test_property_removing_any_declared_unseen_entry_flags_every_leaf_it_alone_covered() -> None:
    """For every `TIER_C_SKIPS` entry, and every real leaf it covers: if no other
    registered name also covers that leaf, removing the entry makes the tool name that
    leaf exactly - the property the team lead asked for, for the "declared unseen" half.
    """
    data = json.loads(real_data_path().read_text())
    leaves, skipped, checked_full = _real_leaves_and_registry()
    registered = skipped | checked_full
    exercised = 0
    for skip_name in skipped:
        covered = [leaf for leaf in leaves if _covers(leaf, skip_name)]
        for leaf in covered:
            others = registered - {skip_name}
            if any(_covers(leaf, other) for other in others):
                continue  # another name still covers it - not this entry's to prove
            exercised += 1
            found = unaccounted_names(data, skipped=skipped - {skip_name})
            assert leaf in found, f"removing {skip_name!r} should have named {leaf!r}, got {found!r}"
    assert exercised > 0, "no TIER_C_SKIPS entry was uniquely responsible for a real leaf - test would be vacuous"


def test_property_removing_any_checked_entry_flags_every_leaf_it_alone_covered() -> None:
    """Same property, for the "checked" half: every `TIER_B_NAMES`/`RADIUS_MAP` entry
    that uniquely covers a real leaf, removed, makes the tool name that leaf.
    """
    data = json.loads(real_data_path().read_text())
    leaves, skipped, checked_full = _real_leaves_and_registry()
    registered = skipped | checked_full
    bare_checked = frozenset(TIER_B_NAMES) | frozenset(RADIUS_MAP)
    exercised = 0
    for bare in bare_checked:
        full_names = [f"type.{bare}", f"sizes.{bare}"]
        covered = [leaf for leaf in leaves if any(_covers(leaf, full) for full in full_names)]
        for leaf in covered:
            others = registered - set(full_names)
            if any(_covers(leaf, other) for other in others):
                continue
            exercised += 1
            found = unaccounted_names(data, checked=bare_checked - {bare})
            assert leaf in found, f"removing {bare!r} should have named {leaf!r}, got {found!r}"
    assert exercised > 0, "no checked entry was uniquely responsible for a real leaf - test would be vacuous"
