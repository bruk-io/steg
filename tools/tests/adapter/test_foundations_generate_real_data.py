"""`render` against the real `crates/ui/design/foundations.json`, not a fixture nobody
has ever pointed at the actual file - the one real-file boundary this generator has.

AC#4's evidence lives here: the data file's own name count against the identifiers the
generator actually emits, printed so it's not just "tests pass."
"""

import json

from steg_tools import workspace
from steg_tools.foundations_generate import assign_identifiers, design_entries, render


def real_data() -> dict[str, object]:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"
    path = root / "crates/ui/design/foundations.json"
    return json.loads(path.read_text())


def test_render_parses_the_real_file_with_no_error() -> None:
    rendered = render(real_data())
    assert rendered  # non-empty: every section contributed something


def test_the_real_files_names_have_no_identifier_collision() -> None:
    names = tuple(name for _, name, _ in design_entries(real_data()))
    assigned = assign_identifiers(names)  # raises IdentifierCollisionError on any collision
    assert len(assigned) == len(names)


def test_ac4_the_data_files_own_name_count_against_the_generated_codes(capsys: object) -> None:
    data = real_data()
    names = tuple(name for _, name, _ in design_entries(data))
    rendered = render(data)
    markers = rendered.count('/// Design name: "')
    print(f"AC#4: {len(names)} names in crates/ui/design/foundations.json, {markers} in generated.rs")  # noqa: T201
    assert markers == len(names)
