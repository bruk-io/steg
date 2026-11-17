"""`run_captured`/`run_live`'s own bound: a real process, in its own process group,
that outlives the bound and spawns a grandchild of its own - proving `killpg` reaches
the whole group a hung step started, not only its direct child
(`tools/src/steg_tools/quiet.py`'s own module doc: the lesson from TASK-1.113's hang).
"""

import sys
import time
from pathlib import Path

from steg_tools.quiet import run_captured, run_live

# Spawns a grandchild that waits half a second, then writes a sentinel file - if the
# whole group is really killed well inside that half second, the sentinel is never
# written. The parent itself then sleeps far longer than any bound this test uses, so
# a step that only kills the direct child (not the group) would still see the
# grandchild finish and write the file.
_OUTLIVES_THE_BOUND_WITH_A_GRANDCHILD = """
import subprocess, sys, time
subprocess.Popen([sys.executable, "-c", (
    "import pathlib, sys, time; time.sleep(0.5); pathlib.Path(sys.argv[1]).write_text('alive')"
), sys.argv[1]])
time.sleep(10)
"""

BOUND_S = 0.2


def test_a_captured_step_past_its_bound_is_killed_group_and_all(tmp_path: Path) -> None:
    sentinel = tmp_path / "sentinel"
    command = [sys.executable, "-c", _OUTLIVES_THE_BOUND_WITH_A_GRANDCHILD, str(sentinel)]

    code, output, seconds = run_captured(command, BOUND_S)

    assert code is None
    assert output == ""
    assert seconds < 2.0, f"a killed step should return close to its {BOUND_S}s bound, took {seconds}s"
    time.sleep(0.6)  # past the grandchild's own 0.5s delay, had it survived
    assert not sentinel.exists(), "the grandchild should have been killed before writing its sentinel"


def test_a_live_step_past_its_bound_is_killed_group_and_all(tmp_path: Path) -> None:
    sentinel = tmp_path / "sentinel"
    command = [sys.executable, "-c", _OUTLIVES_THE_BOUND_WITH_A_GRANDCHILD, str(sentinel)]

    code, seconds = run_live(command, BOUND_S)

    assert code is None
    assert seconds < 2.0, f"a killed step should return close to its {BOUND_S}s bound, took {seconds}s"
    time.sleep(0.6)
    assert not sentinel.exists(), "the grandchild should have been killed before writing its sentinel"


def test_a_captured_step_inside_its_bound_passes_its_output_through() -> None:
    code, output, _ = run_captured([sys.executable, "-c", "print('hi')"], 5.0)
    assert code == 0
    assert output == "hi\n"
