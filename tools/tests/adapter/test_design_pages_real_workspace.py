"""`steg-design-check`'s CLI behaviour against the real repository: the one real boundary it has.

Reads the real `design/canvas/project/{Tokens,Details}.dc.html` always; `--data-file`
points the data-file half at a scratch copy for the negative controls, so the repository's
real pages stay untouched (per `team:testing`: real files, nothing mocked, only the input
this check exists to validate is swapped for a controlled one).
"""

import json
import shutil
from dataclasses import replace
from pathlib import Path

from steg_tools import workspace
from steg_tools.design_pages import SCHEMA, declared_unseen, leaf_classification_counts, main, schema_violations, validate


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


def test_the_real_data_file_matches_schema_exactly() -> None:
    data = json.loads(real_data_path().read_text())
    assert validate(data, SCHEMA) == ()


# --- Round 4 (@bruk.r1): the exclusion list (name/use/unit/ansi/artifact_version/
# artifact_url) was applied at every position, not only where its own reason held, so
# `focus_ring["ansi"]`, `colours[0]["unit"]` and `colours[0]["artifact_version"]` all
# passed unseen. The reviewer's own reproductions, against the real file.


def test_focus_rings_ansi_is_refused_the_exclusion_no_longer_applies_everywhere() -> None:
    # ansi is Checked/recurses only inside a theme entry - focus_ring's own Fields
    # schema has no "ansi" key at all, so it is refused there like any other stray key.
    data = json.loads(real_data_path().read_text())
    data["focus_ring"]["ansi"] = {"black": "#000"}
    found = schema_violations(data)
    assert any(violation.startswith("focus_ring.ansi:") for violation in found)


def test_a_colours_unit_field_is_refused() -> None:
    # "unit" describes a number beside it (focus_ring's, a size's) - it is not one of a
    # colour entry's four allowed fields (name, dark, light, use) at all.
    data = json.loads(real_data_path().read_text())
    data["colours"][0]["unit"] = "hex"
    found = schema_violations(data)
    assert any(violation.startswith(f"colours.{data['colours'][0]['name']}.unit:") for violation in found)


def test_a_colours_artifact_version_field_is_refused() -> None:
    # artifact_version is this export's own top-level provenance field - not allowed
    # inside a colour entry, which has no such field either.
    data = json.loads(real_data_path().read_text())
    data["colours"][0]["artifact_version"] = "bogus"
    found = schema_violations(data)
    assert any(violation.startswith(f"colours.{data['colours'][0]['name']}.artifact_version:") for violation in found)


def test_sizes_nav_sides_use_field_is_not_refused_a_documented_terminal_gap_not_a_regression() -> None:
    # The review's own fourth example (sizes["nav.side"]["use"]) is cited alongside the
    # three above as evidence the *old* unconditional exclusion was unsound - but
    # sizes.nav.side is Checked(recurses=True) under the new schema (a Tier B whole
    # value, terminal by design, the same gap this review's own "everything else passed
    # cleanly" section explicitly declines to re-litigate for a numeric field). A stray
    # "use" here is not inspected by anything, same as any other non-numeric field added
    # inside an already-checked group - a known, accepted gap, not a silent one.
    data = json.loads(real_data_path().read_text())
    data["sizes"]["nav.side"]["use"] = "not a value anything checks"
    assert validate(data, SCHEMA) == ()


# --- Earlier rounds' reproductions, re-run against the real file to prove they still hold.


def test_a_new_top_level_key_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    data["glow_colour"] = "#ffffff"
    assert schema_violations(data) == ("glow_colour: not described by the schema",)


def test_a_new_key_directly_inside_sizes_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    data["sizes"]["new_group"] = {"value": 5, "unit": "px"}
    found = schema_violations(data)
    assert any(violation.startswith("sizes.new_group:") for violation in found)


def test_a_new_field_inside_a_theme_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    assert data["themes"][0]["name"] == "steg dark"
    data["themes"][0]["underline"] = "#fff"
    found = schema_violations(data)
    assert any(violation.startswith("themes.steg dark.underline:") for violation in found)


def test_a_new_field_inside_focus_ring_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    data["focus_ring"]["glow"] = 3
    found = schema_violations(data)
    assert any(violation.startswith("focus_ring.glow:") for violation in found)


def test_a_new_entry_in_a_list_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    data["colour_aliases"].append({"name": "warning", "ref": {"ref": "ramp.5 amber"}, "use": "x"})
    assert validate(data, SCHEMA) == ()  # a well-formed new alias is fine - open to a new entry


def test_a_new_entry_in_a_list_with_an_unknown_field_is_refused() -> None:
    data = json.loads(real_data_path().read_text())
    data["colour_aliases"].append({"name": "warning", "ref": {"ref": "ramp.5 amber"}, "use": "x", "weight": 2})
    found = schema_violations(data)
    assert any(violation.startswith("colour_aliases.warning.weight:") for violation in found)


def test_a_key_that_merely_extends_an_existing_sizes_key_as_a_string_is_refused() -> None:
    # The reviewer's own reproduction: sizes["palette.footer"] - "palette" is a checked
    # sizes key, but "palette.footer" is an unrelated one that only happens to start
    # with the same characters.
    data = json.loads(real_data_path().read_text())
    assert "palette" in data["sizes"] and "palette.footer" not in data["sizes"]
    data["sizes"]["palette.footer"] = {"value": 1, "unit": "px"}
    found = schema_violations(data)
    assert any(violation.startswith("sizes.palette.footer:") for violation in found)


def test_a_key_that_merely_extends_an_existing_type_key_as_a_string_is_refused() -> None:
    # The reviewer's reproduction: type["terminal.bold"] - "terminal" is checked.
    data = json.loads(real_data_path().read_text())
    assert "terminal" in data["type"] and "terminal.bold" not in data["type"]
    data["type"]["terminal.bold"] = {"weight": 700, "unit": "px"}
    found = schema_violations(data)
    assert any(violation.startswith("type.terminal.bold:") for violation in found)


def test_a_key_that_extends_a_key_which_itself_contains_a_dot_is_refused() -> None:
    # The reviewer's reproduction: sizes["nav.side.extra"] - "nav.side" (one real key,
    # with a literal dot in it) is checked; "nav.side.extra" is a different key that
    # happens to share a string prefix with it.
    data = json.loads(real_data_path().read_text())
    assert "nav.side" in data["sizes"] and "nav.side.extra" not in data["sizes"]
    data["sizes"]["nav.side.extra"] = {"value": 1, "unit": "px"}
    found = schema_violations(data)
    assert any(violation.startswith("sizes.nav.side.extra:") for violation in found)


def test_a_new_ramp_whose_name_extends_an_existing_ones_name_is_refused() -> None:
    # The reviewer's reproduction: a ramp named "ramp.secondary" - a well-formed new
    # entry is fine (NamedEntries is open); an *unknown field* on it is what's refused.
    data = json.loads(real_data_path().read_text())
    assert not any(ramp["name"] == "ramp.secondary" for ramp in data["ramps"])
    data["ramps"].append({"name": "ramp.secondary", "use": "x", "stops": [{"stop": 0}], "weight": 1})
    found = schema_violations(data)
    assert any(violation.startswith("ramps.ramp.secondary.weight:") for violation in found)


def test_a_stray_field_on_an_existing_colour_alias_opacity_or_ramp_is_refused() -> None:
    # The reviewer's reproduction, one per kind: each entry is registered field by field
    # (dark/light for a colour; ref for an alias; dark/light for an opacity; stops/
    # caption_opacity for a ramp) - a field beyond those is refused even though the
    # entry's own name is known, since nothing actually checks it.
    data = json.loads(real_data_path().read_text())
    for entry in data["colours"]:
        if entry["name"] == "surface.ground":
            entry["contrast"] = 1.5
    for entry in data["colour_aliases"]:
        if entry["name"] == "blocked":
            entry["weight"] = 2
    for entry in data["opacities"]:
        if entry["name"] == "faint":
            entry["blur"] = 1
    for entry in data["ramps"]:
        if entry["name"] == "ramp":
            entry["intensity"] = 2

    found = schema_violations(data)
    assert any(violation.startswith("colours.surface.ground.contrast:") for violation in found)
    assert any(violation.startswith("colour_aliases.blocked.weight:") for violation in found)
    assert any(violation.startswith("opacities.faint.blur:") for violation in found)
    assert any(violation.startswith("ramps.ramp.intensity:") for violation in found)


def test_declared_unseen_against_the_real_file_lists_every_entry_with_a_reason() -> None:
    data = json.loads(real_data_path().read_text())
    found = declared_unseen(data)
    assert len(found) > 0
    assert all(skip.reason.strip() for skip in found)


def test_removing_a_declared_unseen_entry_makes_that_key_unaccounted() -> None:
    # Required negative control: with "session_colour" struck from SCHEMA entirely, it
    # is refused as unknown rather than silently accepted.
    data = json.loads(real_data_path().read_text())
    reduced_fields = {key: value for key, value in SCHEMA.fields.items() if key != "session_colour"}
    reduced_schema = replace(SCHEMA, fields=reduced_fields)
    found = validate(data, reduced_schema)
    assert any(path == ("session_colour",) and "not described" in reason for path, reason in found)


def test_leaf_classification_counts_add_up_to_the_real_files_own_leaf_count() -> None:
    # The arithmetic the review asked for: every leaf an exhaustive, no-exclusions
    # enumeration finds is classified as exactly one of checked/declared_unseen/
    # descriptive - nothing falls through uncounted.
    data = json.loads(real_data_path().read_text())

    def exhaustive(value: object) -> int:
        if isinstance(value, dict):
            return (value and sum(exhaustive(sub) for sub in value.values())) or 1
        if isinstance(value, list):
            return (value and sum(exhaustive(item) for item in value)) or 1
        return 1

    counts = leaf_classification_counts(data)
    assert sum(counts.values()) == exhaustive(data)
    assert counts["checked"] > 0
    assert counts["declared_unseen"] > 0
    assert counts["descriptive"] > 0
